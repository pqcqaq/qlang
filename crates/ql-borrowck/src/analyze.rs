use std::collections::{HashSet, VecDeque};

use ql_ast::ReceiverKind;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{self as hir, Function, ItemKind, Param};
use ql_mir::{
    BasicBlockId, BodyOwner, Constant, LocalId as MirLocalId, LocalOrigin, MirBody, MirModule,
    Operand, Place, ProjectionElem, Rvalue, StatementKind, TerminatorKind,
};
use ql_resolve::ResolutionMap;
use ql_typeck::{Ty, TypeckResult, lower_type};

use crate::{
    BlockFacts, BodyFacts, BorrowckResult, LocalEvent, LocalEventKind, LocalState, MoveCertainty,
    MoveInfo, MoveOrigin, MoveReason,
};

pub fn analyze_module(
    hir: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    mir: &MirModule,
) -> BorrowckResult {
    let mut result = BorrowckResult::default();

    for &body_id in mir.bodies() {
        let body = mir.body(body_id);
        let Some(function) = function_for_owner(hir, body.owner) else {
            continue;
        };

        let analyzer = BodyAnalyzer::new(hir, resolution, typeck, body, function);
        let (facts, diagnostics) = analyzer.analyze();
        result.push_body(facts);
        for diagnostic in diagnostics {
            result.push_diagnostic(diagnostic);
        }
    }

    result
}

struct BodyAnalyzer<'a> {
    hir: &'a hir::Module,
    resolution: &'a ResolutionMap,
    typeck: &'a TypeckResult,
    body: &'a MirBody,
    function: &'a Function,
    receiver_ty: Option<Ty>,
}

impl<'a> BodyAnalyzer<'a> {
    fn new(
        hir: &'a hir::Module,
        resolution: &'a ResolutionMap,
        typeck: &'a TypeckResult,
        body: &'a MirBody,
        function: &'a Function,
    ) -> Self {
        let receiver_ty = receiver_target_ty(hir, resolution, body.owner);

        Self {
            hir,
            resolution,
            typeck,
            body,
            function,
            receiver_ty,
        }
    }

    fn analyze(&self) -> (BodyFacts, Vec<Diagnostic>) {
        let local_count = self.body.locals().len();
        let initial_state = vec![LocalState::Unavailable; local_count];
        let mut entry_states = vec![None; self.body.blocks().len()];
        let mut exit_states = vec![None; self.body.blocks().len()];
        entry_states[self.body.entry.index()] = Some(initial_state);

        let mut worklist = VecDeque::from([self.body.entry]);

        while let Some(block_id) = worklist.pop_front() {
            let entry_state = entry_states[block_id.index()]
                .clone()
                .expect("scheduled blocks should have an entry state");
            let exit_state = self.transfer_block(block_id, entry_state.clone(), None);
            let changed_exit = exit_states[block_id.index()]
                .as_ref()
                .is_none_or(|previous| previous != &exit_state);
            if changed_exit {
                exit_states[block_id.index()] = Some(exit_state.clone());
            }

            for successor in successors(&self.body.block(block_id).terminator.kind) {
                let (merged, changed) =
                    merge_state_vec(entry_states[successor.index()].as_deref(), &exit_state);
                if changed {
                    entry_states[successor.index()] = Some(merged);
                    worklist.push_back(successor);
                }
            }
        }

        let mut reporter = Reporter::default();
        let mut blocks = Vec::with_capacity(self.body.blocks().len());
        for (index, block_id) in self.body.block_ids().enumerate() {
            let block = self.body.block(block_id);
            let entry = entry_states[index]
                .clone()
                .unwrap_or_else(|| vec![LocalState::Unavailable; local_count]);
            let exit = self.transfer_block(block_id, entry.clone(), Some(&mut reporter));
            let expected = exit_states[index]
                .clone()
                .unwrap_or_else(|| vec![LocalState::Unavailable; local_count]);
            debug_assert_eq!(exit, expected, "borrowck block transfer should be stable");

            blocks.push(BlockFacts {
                block_index: index,
                entry_states: entry,
                exit_states: expected,
            });

            let _ = block;
        }

        (
            BodyFacts {
                owner: self.body.owner,
                name: self.body.name.clone(),
                blocks,
                events: reporter.events,
            },
            reporter.diagnostics,
        )
    }

    fn transfer_block(
        &self,
        block_id: BasicBlockId,
        mut states: Vec<LocalState>,
        mut reporter: Option<&mut Reporter>,
    ) -> Vec<LocalState> {
        let block = self.body.block(block_id);

        for statement_id in &block.statements {
            let statement = self.body.statement(*statement_id);
            self.apply_statement(
                &mut states,
                statement.span,
                &statement.kind,
                reporter.as_deref_mut(),
            );
        }

        self.apply_terminator(
            &mut states,
            block.terminator.span,
            &block.terminator.kind,
            reporter,
        );

        states
    }

    fn apply_statement(
        &self,
        states: &mut [LocalState],
        span: ql_span::Span,
        statement: &StatementKind,
        reporter: Option<&mut Reporter>,
    ) {
        match statement {
            StatementKind::Assign { place, value } => {
                let mut reporter = reporter;
                self.apply_rvalue(states, value, span, reporter.as_deref_mut());
                if place.projections.is_empty() {
                    self.record_event(
                        reporter.as_deref_mut(),
                        span,
                        place.base,
                        LocalEventKind::Write,
                    );
                    states[place.base.index()] = LocalState::Available;
                } else {
                    self.read_place(states, place, span, reporter);
                }
            }
            StatementKind::BindPattern { source, .. } => {
                self.read_operand(states, source, span, reporter);
            }
            StatementKind::Eval { value } => self.apply_rvalue(states, value, span, reporter),
            StatementKind::StorageLive { local } => states[local.index()] = LocalState::Available,
            StatementKind::StorageDead { local } => states[local.index()] = LocalState::Unavailable,
            StatementKind::RegisterCleanup { .. } | StatementKind::RunCleanup { .. } => {}
        }
    }

    fn apply_terminator(
        &self,
        states: &mut [LocalState],
        span: ql_span::Span,
        terminator: &TerminatorKind,
        reporter: Option<&mut Reporter>,
    ) {
        match terminator {
            TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Terminate => {}
            TerminatorKind::Branch { condition, .. } => {
                self.read_operand(states, condition, span, reporter);
            }
            TerminatorKind::Match { scrutinee, .. } => {
                self.read_operand(states, scrutinee, span, reporter);
            }
            TerminatorKind::ForLoop { iterable, .. } => {
                self.read_operand(states, iterable, span, reporter);
            }
        }
    }

    fn apply_rvalue(
        &self,
        states: &mut [LocalState],
        value: &Rvalue,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        match value {
            Rvalue::Use(operand) => self.read_operand(states, operand, span, reporter),
            Rvalue::Tuple(items) | Rvalue::Array(items) => {
                let mut reporter = reporter;
                for item in items {
                    self.read_operand(states, item, span, reporter.as_deref_mut());
                }
            }
            Rvalue::Call { callee, args } => {
                let mut reporter = reporter;
                if !self.consume_move_receiver(states, callee, span, reporter.as_deref_mut()) {
                    self.read_operand(states, callee, span, reporter.as_deref_mut());
                }
                for arg in args {
                    self.read_operand(states, &arg.value, span, reporter.as_deref_mut());
                }
            }
            Rvalue::Binary { left, right, .. } => {
                let mut reporter = reporter;
                self.read_operand(states, left, span, reporter.as_deref_mut());
                self.read_operand(states, right, span, reporter);
            }
            Rvalue::Unary { operand, .. } | Rvalue::Question(operand) => {
                self.read_operand(states, operand, span, reporter)
            }
            Rvalue::AggregateStruct { fields, .. } => {
                let mut reporter = reporter;
                for field in fields {
                    self.read_operand(states, &field.value, span, reporter.as_deref_mut());
                }
            }
            Rvalue::Closure { .. } | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn read_operand(
        &self,
        states: &mut [LocalState],
        operand: &Operand,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        match operand {
            Operand::Place(place) => self.read_place(states, place, span, reporter),
            Operand::Constant(constant) => self.read_constant(states, constant, span, reporter),
        }
    }

    fn read_constant(
        &self,
        states: &mut [LocalState],
        constant: &Constant,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        let _ = (states, span, reporter, constant);
    }

    fn read_place(
        &self,
        states: &mut [LocalState],
        place: &Place,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        let mut reporter = reporter;
        self.check_moved_use(states, place.base, span, reporter.as_deref_mut());
        self.record_event(
            reporter.as_deref_mut(),
            span,
            place.base,
            LocalEventKind::Read,
        );

        for projection in &place.projections {
            if let ProjectionElem::Index(operand) = projection {
                self.read_operand(states, operand, span, reporter.as_deref_mut());
            }
        }
    }

    fn consume_move_receiver(
        &self,
        states: &mut [LocalState],
        callee: &Operand,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) -> bool {
        let Operand::Place(place) = callee else {
            return false;
        };
        // P3.2 intentionally only models direct-local receivers. Projection-sensitive
        // consumption needs a later place-aware analysis instead of ad hoc rules here.
        let Some(ProjectionElem::Field(method_name)) = place.projections.last() else {
            return false;
        };
        if place.projections.len() != 1 {
            return false;
        }

        let Some(receiver_ty) = self.local_ty(place.base) else {
            return false;
        };
        let Some(reason) = self.unique_move_receiver_reason(&receiver_ty, method_name) else {
            return false;
        };

        let mut reporter = reporter;
        self.check_moved_use(states, place.base, span, reporter.as_deref_mut());
        self.record_event(
            reporter,
            span,
            place.base,
            LocalEventKind::Consume(reason.clone()),
        );
        states[place.base.index()] = LocalState::Moved(MoveInfo {
            certainty: MoveCertainty::Definite,
            origins: vec![MoveOrigin { span, reason }],
        });
        true
    }

    fn check_moved_use(
        &self,
        states: &[LocalState],
        local: MirLocalId,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        let Some(reporter) = reporter else {
            return;
        };
        let LocalState::Moved(info) = &states[local.index()] else {
            return;
        };
        if info.origins.is_empty() {
            return;
        }

        let name = &self.body.local(local).name;
        let key = DiagnosticKey {
            local,
            use_span: span,
            certainty: info.certainty,
            origin_spans: info.origins.iter().map(|origin| origin.span).collect(),
        };
        if !reporter.emitted.insert(key) {
            return;
        }

        let mut diagnostic = match info.certainty {
            MoveCertainty::Definite => {
                Diagnostic::error(format!("local `{name}` was used after move"))
            }
            MoveCertainty::Maybe => Diagnostic::error(format!(
                "local `{name}` may have been moved on another control-flow path"
            )),
        }
        .with_label(Label::new(span).with_message("use here"));

        for origin in &info.origins {
            diagnostic = diagnostic.with_label(
                Label::new(origin.span)
                    .secondary()
                    .with_message(render_move_origin(origin)),
            );
        }

        if info.certainty == MoveCertainty::Maybe {
            diagnostic = diagnostic.with_note(
                "this local is only known to be moved on some incoming paths in the current checker",
            );
        }

        reporter.diagnostics.push(diagnostic);
    }

    fn unique_move_receiver_reason(
        &self,
        receiver_ty: &Ty,
        method_name: &str,
    ) -> Option<MoveReason> {
        let mut matched_move = false;
        let mut matched_non_move = false;
        let mut total_candidates = 0usize;

        for &item_id in &self.hir.items {
            match &self.hir.item(item_id).kind {
                ItemKind::Impl(impl_block) => {
                    let target_ty = lower_type(self.hir, self.resolution, impl_block.target);
                    if receiver_ty.compatible_with(&target_ty) {
                        self.accumulate_method_candidates(
                            &impl_block.methods,
                            method_name,
                            &mut total_candidates,
                            &mut matched_move,
                            &mut matched_non_move,
                        );
                    }
                }
                ItemKind::Extend(extend_block) => {
                    let target_ty = lower_type(self.hir, self.resolution, extend_block.target);
                    if receiver_ty.compatible_with(&target_ty) {
                        self.accumulate_method_candidates(
                            &extend_block.methods,
                            method_name,
                            &mut total_candidates,
                            &mut matched_move,
                            &mut matched_non_move,
                        );
                    }
                }
                _ => {}
            }
        }

        if total_candidates == 1 && matched_move && !matched_non_move {
            Some(MoveReason::MoveSelfMethod {
                method_name: method_name.to_owned(),
            })
        } else {
            None
        }
    }

    fn accumulate_method_candidates(
        &self,
        methods: &[Function],
        method_name: &str,
        total_candidates: &mut usize,
        matched_move: &mut bool,
        matched_non_move: &mut bool,
    ) {
        for method in methods {
            if method.name != method_name {
                continue;
            }

            *total_candidates += 1;
            let is_move = matches!(
                method.params.first(),
                Some(Param::Receiver(receiver)) if receiver.kind == ReceiverKind::Move
            );
            if is_move {
                *matched_move = true;
            } else {
                *matched_non_move = true;
            }
        }
    }

    fn local_ty(&self, local: MirLocalId) -> Option<Ty> {
        match &self.body.local(local).origin {
            LocalOrigin::Binding(hir_local) => self.typeck.local_ty(*hir_local).cloned(),
            LocalOrigin::Param { index } => match &self.function.params[*index] {
                Param::Regular(param) => Some(lower_type(self.hir, self.resolution, param.ty)),
                Param::Receiver(_) => self.receiver_ty.clone(),
            },
            LocalOrigin::Receiver => self.receiver_ty.clone(),
            LocalOrigin::ReturnSlot | LocalOrigin::Temp { .. } => None,
        }
    }

    fn record_event(
        &self,
        reporter: Option<&mut Reporter>,
        span: ql_span::Span,
        local: MirLocalId,
        kind: LocalEventKind,
    ) {
        let Some(reporter) = reporter else {
            return;
        };
        reporter.events.push(LocalEvent { span, local, kind });
    }
}

#[derive(Default)]
struct Reporter {
    events: Vec<LocalEvent>,
    diagnostics: Vec<Diagnostic>,
    emitted: HashSet<DiagnosticKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiagnosticKey {
    local: MirLocalId,
    use_span: ql_span::Span,
    certainty: MoveCertainty,
    origin_spans: Vec<ql_span::Span>,
}

fn merge_state_vec(
    existing: Option<&[LocalState]>,
    incoming: &[LocalState],
) -> (Vec<LocalState>, bool) {
    match existing {
        None => (incoming.to_vec(), true),
        Some(existing) => {
            let merged = existing
                .iter()
                .zip(incoming)
                .map(|(existing, incoming)| merge_local_state(existing, incoming))
                .collect::<Vec<_>>();
            let changed = merged != existing;
            (merged, changed)
        }
    }
}

fn merge_local_state(left: &LocalState, right: &LocalState) -> LocalState {
    match (left, right) {
        (LocalState::Moved(left_move), LocalState::Moved(right_move)) => {
            LocalState::Moved(MoveInfo {
                certainty: match (left_move.certainty, right_move.certainty) {
                    (MoveCertainty::Definite, MoveCertainty::Definite) => MoveCertainty::Definite,
                    _ => MoveCertainty::Maybe,
                },
                origins: merge_origins(&left_move.origins, &right_move.origins),
            })
        }
        (LocalState::Moved(move_info), LocalState::Available)
        | (LocalState::Available, LocalState::Moved(move_info))
        | (LocalState::Moved(move_info), LocalState::Unavailable)
        | (LocalState::Unavailable, LocalState::Moved(move_info)) => LocalState::Moved(MoveInfo {
            certainty: MoveCertainty::Maybe,
            origins: move_info.origins.clone(),
        }),
        // This slice only tracks moved-vs-usable facts. It deliberately does not
        // diagnose uninitialized/dead uses yet, so availability wins over absence here.
        (LocalState::Available, LocalState::Available)
        | (LocalState::Available, LocalState::Unavailable)
        | (LocalState::Unavailable, LocalState::Available) => LocalState::Available,
        (LocalState::Unavailable, LocalState::Unavailable) => LocalState::Unavailable,
    }
}

fn merge_origins(left: &[MoveOrigin], right: &[MoveOrigin]) -> Vec<MoveOrigin> {
    let mut origins = left.to_vec();
    for origin in right {
        if !origins.contains(origin) {
            origins.push(origin.clone());
        }
    }
    origins.sort_by_key(|origin| (origin.span.start, origin.span.end));
    origins
}

fn render_move_origin(origin: &MoveOrigin) -> String {
    match &origin.reason {
        MoveReason::MoveSelfMethod { method_name } => {
            format!("consumed here by `move self` method `{method_name}`")
        }
    }
}

fn successors(terminator: &TerminatorKind) -> Vec<BasicBlockId> {
    match terminator {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::Branch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        TerminatorKind::Match {
            arms, else_target, ..
        } => arms
            .iter()
            .map(|arm| arm.target)
            .chain(std::iter::once(*else_target))
            .collect(),
        TerminatorKind::ForLoop {
            body_target,
            exit_target,
            ..
        } => vec![*body_target, *exit_target],
        TerminatorKind::Return | TerminatorKind::Terminate => Vec::new(),
    }
}

fn function_for_owner(hir: &hir::Module, owner: BodyOwner) -> Option<&Function> {
    match owner {
        BodyOwner::Item(item) => match &hir.item(item).kind {
            ItemKind::Function(function) => Some(function),
            _ => None,
        },
        BodyOwner::TraitMethod { item, index } => match &hir.item(item).kind {
            ItemKind::Trait(trait_item) => trait_item.methods.get(index),
            _ => None,
        },
        BodyOwner::ImplMethod { item, index } => match &hir.item(item).kind {
            ItemKind::Impl(impl_item) => impl_item.methods.get(index),
            _ => None,
        },
        BodyOwner::ExtendMethod { item, index } => match &hir.item(item).kind {
            ItemKind::Extend(extend_item) => extend_item.methods.get(index),
            _ => None,
        },
    }
}

fn receiver_target_ty(
    hir: &hir::Module,
    resolution: &ResolutionMap,
    owner: BodyOwner,
) -> Option<Ty> {
    match owner {
        BodyOwner::ImplMethod { item, .. } => match &hir.item(item).kind {
            ItemKind::Impl(impl_item) => Some(lower_type(hir, resolution, impl_item.target)),
            _ => None,
        },
        BodyOwner::ExtendMethod { item, .. } => match &hir.item(item).kind {
            ItemKind::Extend(extend_item) => Some(lower_type(hir, resolution, extend_item.target)),
            _ => None,
        },
        BodyOwner::Item(_) | BodyOwner::TraitMethod { .. } => None,
    }
}
