use std::collections::{HashMap, HashSet, VecDeque};

use ql_ast::{BinaryOp, ReceiverKind};
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{self as hir, Function, ItemKind, Param};
use ql_mir::{
    BasicBlockId, BodyOwner, CleanupId, CleanupKind, Constant, LocalId as MirLocalId, LocalOrigin,
    MirBody, MirModule, Operand, Place, ProjectionElem, Rvalue, StatementKind, TerminatorKind,
};
use ql_resolve::{ResolutionMap, ValueResolution};
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
    binding_locals: HashMap<hir::LocalId, MirLocalId>,
    param_locals: HashMap<usize, MirLocalId>,
    receiver_local: Option<MirLocalId>,
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
        let mut binding_locals = HashMap::new();
        let mut param_locals = HashMap::new();
        let mut receiver_local = None;

        for local_id in body.local_ids() {
            let local = body.local(local_id);
            match &local.origin {
                LocalOrigin::Binding(hir_local) => {
                    binding_locals.insert(*hir_local, local_id);
                }
                LocalOrigin::Param { index } => {
                    param_locals.insert(*index, local_id);
                }
                LocalOrigin::Receiver => {
                    receiver_local = Some(local_id);
                }
                LocalOrigin::ReturnSlot | LocalOrigin::Temp { .. } => {}
            }
        }

        Self {
            hir,
            resolution,
            typeck,
            body,
            function,
            receiver_ty,
            binding_locals,
            param_locals,
            receiver_local,
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
                    self.write_local(states, place.base, span, reporter.as_deref_mut());
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
            StatementKind::RegisterCleanup { .. } => {}
            StatementKind::RunCleanup { cleanup } => {
                self.apply_cleanup(states, *cleanup, reporter);
            }
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
                let pending_consume = self.classify_move_receiver_operand(
                    states,
                    callee,
                    span,
                    reporter.as_deref_mut(),
                );
                if pending_consume.is_none() {
                    self.read_operand(states, callee, span, reporter.as_deref_mut());
                }
                for arg in args {
                    self.read_operand(states, &arg.value, span, reporter.as_deref_mut());
                }
                if let Some((local, reason)) = pending_consume {
                    self.apply_consume(states, local, span, reason, reporter);
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
            Rvalue::Closure {
                is_move, captures, ..
            } => {
                self.apply_closure_capture_effects(states, *is_move, captures, reporter);
            }
            Rvalue::OpaqueExpr(_) => {}
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
        self.read_local(
            states,
            place.base,
            UseSite::normal(span),
            reporter.as_deref_mut(),
        );

        for projection in &place.projections {
            if let ProjectionElem::Index(operand) = projection {
                self.read_operand(states, operand, span, reporter.as_deref_mut());
            }
        }
    }

    fn classify_move_receiver_operand(
        &self,
        states: &[LocalState],
        callee: &Operand,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) -> Option<(MirLocalId, MoveReason)> {
        let Operand::Place(place) = callee else {
            return None;
        };
        // P3.2 intentionally only models direct-local receivers. Projection-sensitive
        // consumption needs a later place-aware analysis instead of ad hoc rules here.
        let Some(ProjectionElem::Field(method_name)) = place.projections.last() else {
            return None;
        };
        if place.projections.len() != 1 {
            return None;
        }

        let receiver_ty = self.local_ty(place.base)?;
        let reason = self.unique_move_receiver_reason(&receiver_ty, method_name)?;

        self.check_moved_use(states, place.base, UseSite::normal(span), reporter);

        Some((place.base, reason))
    }

    fn apply_consume(
        &self,
        states: &mut [LocalState],
        local: MirLocalId,
        span: ql_span::Span,
        reason: MoveReason,
        reporter: Option<&mut Reporter>,
    ) {
        self.record_event(
            reporter,
            span,
            local,
            LocalEventKind::Consume(reason.clone()),
        );
        let origin = MoveOrigin { span, reason };
        let next = match &states[local.index()] {
            LocalState::Moved(existing) => LocalState::Moved(MoveInfo {
                certainty: MoveCertainty::Definite,
                origins: merge_origins(&existing.origins, std::slice::from_ref(&origin)),
            }),
            LocalState::Unavailable | LocalState::Available => LocalState::Moved(MoveInfo {
                certainty: MoveCertainty::Definite,
                origins: vec![origin],
            }),
        };
        states[local.index()] = next;
    }

    fn read_local(
        &self,
        states: &[LocalState],
        local: MirLocalId,
        use_site: UseSite,
        reporter: Option<&mut Reporter>,
    ) {
        let mut reporter = reporter;
        self.check_moved_use(states, local, use_site, reporter.as_deref_mut());
        self.record_event(reporter, use_site.span, local, LocalEventKind::Read);
    }

    fn write_local(
        &self,
        states: &mut [LocalState],
        local: MirLocalId,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
    ) {
        self.record_event(reporter, span, local, LocalEventKind::Write);
        states[local.index()] = LocalState::Available;
    }

    fn apply_closure_capture_effects(
        &self,
        states: &mut [LocalState],
        is_move: bool,
        captures: &[ql_mir::ClosureCapture],
        reporter: Option<&mut Reporter>,
    ) {
        let mut reporter = reporter;

        for capture in captures {
            if is_move {
                self.check_moved_use(
                    states,
                    capture.local,
                    UseSite::move_closure_capture(capture.span),
                    reporter.as_deref_mut(),
                );
                self.apply_consume(
                    states,
                    capture.local,
                    capture.span,
                    MoveReason::MoveClosureCapture,
                    reporter.as_deref_mut(),
                );
            } else {
                self.read_local(
                    states,
                    capture.local,
                    UseSite::closure_capture(capture.span),
                    reporter.as_deref_mut(),
                );
            }
        }
    }

    fn check_moved_use(
        &self,
        states: &[LocalState],
        local: MirLocalId,
        use_site: UseSite,
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
            use_span: use_site.span,
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
        .with_label(Label::new(use_site.span).with_message(use_site.label));

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
        if let Some(note) = use_site.note {
            diagnostic = diagnostic.with_note(note);
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

    fn local_for_resolution(&self, resolution: &ValueResolution) -> Option<MirLocalId> {
        match resolution {
            ValueResolution::Local(local) => self.binding_locals.get(local).copied(),
            ValueResolution::Param(binding) => self.param_locals.get(&binding.index).copied(),
            ValueResolution::SelfValue => self.receiver_local,
            ValueResolution::Item(_) | ValueResolution::Import(_) => None,
        }
    }

    fn direct_local_for_expr(&self, expr_id: hir::ExprId) -> Option<MirLocalId> {
        match &self.hir.expr(expr_id).kind {
            hir::ExprKind::Name(_) => self
                .resolution
                .expr_resolution(expr_id)
                .and_then(|resolution| self.local_for_resolution(resolution)),
            _ => None,
        }
    }

    fn classify_move_receiver_expr(
        &self,
        states: &[LocalState],
        callee: hir::ExprId,
        span: ql_span::Span,
        reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> Option<(MirLocalId, MoveReason)> {
        let hir::ExprKind::Member { object, field } = &self.hir.expr(callee).kind else {
            return None;
        };
        let local = self.direct_local_for_expr(*object)?;
        let receiver_ty = self.local_ty(local)?;
        let reason = self.unique_move_receiver_reason(&receiver_ty, field)?;

        self.check_moved_use(states, local, use_site.with_span(span), reporter);
        Some((local, reason))
    }

    fn apply_cleanup(
        &self,
        states: &mut [LocalState],
        cleanup: CleanupId,
        reporter: Option<&mut Reporter>,
    ) {
        match &self.body.cleanup(cleanup).kind {
            CleanupKind::Defer { expr } => {
                let result = self.eval_cleanup_expr(
                    states.to_vec(),
                    *expr,
                    reporter,
                    UseSite::deferred(self.hir.expr(*expr).span),
                );
                states.clone_from_slice(&result.states);
            }
        }
    }

    fn eval_cleanup_expr(
        &self,
        states: Vec<LocalState>,
        expr_id: hir::ExprId,
        reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> CleanupEval {
        let expr = self.hir.expr(expr_id);
        match &expr.kind {
            hir::ExprKind::Name(_) => {
                if let Some(local) = self.direct_local_for_expr(expr_id) {
                    self.read_local(&states, local, use_site.with_span(expr.span), reporter);
                }
                CleanupEval::cont(states)
            }
            hir::ExprKind::Integer(_)
            | hir::ExprKind::String { .. }
            | hir::ExprKind::Bool(_)
            | hir::ExprKind::NoneLiteral
            | hir::ExprKind::Closure { .. } => CleanupEval::cont(states),
            hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => {
                self.eval_cleanup_exprs(states, items, reporter, use_site)
            }
            hir::ExprKind::Block(block) | hir::ExprKind::Unsafe(block) => {
                self.eval_cleanup_block(states, *block, reporter, use_site)
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let mut reporter = reporter;
                let condition_eval = self.eval_cleanup_expr(
                    states,
                    *condition,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*condition).span),
                );
                if !condition_eval.continues {
                    return condition_eval;
                }

                let then_eval = self.eval_cleanup_block(
                    condition_eval.states.clone(),
                    *then_branch,
                    reporter.as_deref_mut(),
                    use_site,
                );
                let else_eval = if let Some(else_expr) = else_branch {
                    self.eval_cleanup_expr(
                        condition_eval.states,
                        *else_expr,
                        reporter,
                        use_site.with_span(self.hir.expr(*else_expr).span),
                    )
                } else {
                    CleanupEval::cont(condition_eval.states)
                };
                merge_cleanup_branches(then_eval, else_eval)
            }
            hir::ExprKind::Match { value, arms } => {
                let mut reporter = reporter;
                let scrutinee_eval = self.eval_cleanup_expr(
                    states,
                    *value,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*value).span),
                );
                if !scrutinee_eval.continues {
                    return scrutinee_eval;
                }

                let mut arm_results = Vec::with_capacity(arms.len());
                for arm in arms {
                    let mut arm_eval = CleanupEval::cont(scrutinee_eval.states.clone());
                    if let Some(guard) = arm.guard {
                        arm_eval = self.eval_cleanup_expr(
                            arm_eval.states,
                            guard,
                            reporter.as_deref_mut(),
                            use_site.with_span(self.hir.expr(guard).span),
                        );
                    }
                    if arm_eval.continues {
                        arm_eval = self.eval_cleanup_expr(
                            arm_eval.states,
                            arm.body,
                            reporter.as_deref_mut(),
                            use_site.with_span(self.hir.expr(arm.body).span),
                        );
                    }
                    arm_results.push(arm_eval);
                }

                merge_cleanup_branch_set(scrutinee_eval.states, arm_results)
            }
            hir::ExprKind::Call { callee, args } => {
                let mut reporter = reporter;
                let mut states = states;
                let pending_consume = self.classify_move_receiver_expr(
                    &states,
                    *callee,
                    expr.span,
                    reporter.as_deref_mut(),
                    use_site,
                );
                if pending_consume.is_none() {
                    let callee_eval = self.eval_cleanup_expr(
                        states,
                        *callee,
                        reporter.as_deref_mut(),
                        use_site.with_span(self.hir.expr(*callee).span),
                    );
                    if !callee_eval.continues {
                        return callee_eval;
                    }
                    states = callee_eval.states;
                }
                for arg in args {
                    let value = match arg {
                        hir::CallArg::Positional(value) => *value,
                        hir::CallArg::Named { value, .. } => *value,
                    };
                    let arg_eval = self.eval_cleanup_expr(
                        states,
                        value,
                        reporter.as_deref_mut(),
                        use_site.with_span(self.hir.expr(value).span),
                    );
                    if !arg_eval.continues {
                        return arg_eval;
                    }
                    states = arg_eval.states;
                }
                if let Some((local, reason)) = pending_consume {
                    self.apply_consume(&mut states, local, expr.span, reason, reporter);
                }
                CleanupEval::cont(states)
            }
            hir::ExprKind::Member { object, .. } => self.eval_cleanup_expr(
                states,
                *object,
                reporter,
                use_site.with_span(self.hir.expr(*object).span),
            ),
            hir::ExprKind::Bracket { target, items } => {
                let mut reporter = reporter;
                let target_eval = self.eval_cleanup_expr(
                    states,
                    *target,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*target).span),
                );
                if !target_eval.continues {
                    return target_eval;
                }
                self.eval_cleanup_exprs(target_eval.states, items, reporter, use_site)
            }
            hir::ExprKind::StructLiteral { fields, .. } => self.eval_cleanup_exprs(
                states,
                &fields.iter().map(|field| field.value).collect::<Vec<_>>(),
                reporter,
                use_site,
            ),
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let mut reporter = reporter;
                let target_eval = self.eval_cleanup_assign_target(
                    states,
                    *left,
                    reporter.as_deref_mut(),
                    use_site,
                );
                if !target_eval.continues {
                    return CleanupEval::stop(target_eval.states);
                }

                let mut states = target_eval.states;
                let value_eval = self.eval_cleanup_expr(
                    states,
                    *right,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*right).span),
                );
                if !value_eval.continues {
                    return value_eval;
                }
                states = value_eval.states;
                if let Some(local) = target_eval.root_local {
                    self.write_local(&mut states, local, expr.span, reporter);
                }
                CleanupEval::cont(states)
            }
            hir::ExprKind::Binary { left, right, .. } => {
                let mut reporter = reporter;
                let left_eval = self.eval_cleanup_expr(
                    states,
                    *left,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*left).span),
                );
                if !left_eval.continues {
                    return left_eval;
                }
                self.eval_cleanup_expr(
                    left_eval.states,
                    *right,
                    reporter,
                    use_site.with_span(self.hir.expr(*right).span),
                )
            }
            hir::ExprKind::Unary { expr: inner, .. } | hir::ExprKind::Question(inner) => self
                .eval_cleanup_expr(
                    states,
                    *inner,
                    reporter,
                    use_site.with_span(self.hir.expr(*inner).span),
                ),
        }
    }

    fn eval_cleanup_exprs(
        &self,
        mut states: Vec<LocalState>,
        exprs: &[hir::ExprId],
        mut reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> CleanupEval {
        for expr in exprs {
            let eval = self.eval_cleanup_expr(
                states,
                *expr,
                reporter.as_deref_mut(),
                use_site.with_span(self.hir.expr(*expr).span),
            );
            if !eval.continues {
                return eval;
            }
            states = eval.states;
        }
        CleanupEval::cont(states)
    }

    fn eval_cleanup_block(
        &self,
        mut states: Vec<LocalState>,
        block_id: hir::BlockId,
        mut reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> CleanupEval {
        let block = self.hir.block(block_id);
        for stmt_id in &block.statements {
            let eval = self.eval_cleanup_stmt(states, *stmt_id, reporter.as_deref_mut(), use_site);
            if !eval.continues {
                return eval;
            }
            states = eval.states;
        }

        if let Some(tail) = block.tail {
            self.eval_cleanup_expr(
                states,
                tail,
                reporter,
                use_site.with_span(self.hir.expr(tail).span),
            )
        } else {
            CleanupEval::cont(states)
        }
    }

    fn eval_cleanup_stmt(
        &self,
        states: Vec<LocalState>,
        stmt_id: hir::StmtId,
        mut reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> CleanupEval {
        let stmt = self.hir.stmt(stmt_id);
        match &stmt.kind {
            hir::StmtKind::Let { value, .. } => self.eval_cleanup_expr(
                states,
                *value,
                reporter,
                use_site.with_span(self.hir.expr(*value).span),
            ),
            hir::StmtKind::Return(Some(expr)) => {
                let eval = self.eval_cleanup_expr(
                    states,
                    *expr,
                    reporter,
                    use_site.with_span(self.hir.expr(*expr).span),
                );
                CleanupEval::stop(eval.states)
            }
            hir::StmtKind::Return(None) | hir::StmtKind::Break | hir::StmtKind::Continue => {
                CleanupEval::stop(states)
            }
            // Nested `defer` inside a deferred cleanup needs dedicated runtime modeling later.
            hir::StmtKind::Defer(_) => CleanupEval::cont(states),
            hir::StmtKind::While { condition, body } => {
                let condition_eval = self.eval_cleanup_expr(
                    states,
                    *condition,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*condition).span),
                );
                if !condition_eval.continues {
                    return condition_eval;
                }
                let body_eval = self.eval_cleanup_block(
                    condition_eval.states.clone(),
                    *body,
                    reporter,
                    use_site,
                );
                CleanupEval::cont(
                    merge_state_vec(Some(&condition_eval.states), &body_eval.states).0,
                )
            }
            hir::StmtKind::Loop { body } => {
                self.eval_cleanup_block(states, *body, reporter, use_site)
            }
            hir::StmtKind::For { iterable, body, .. } => {
                let iterable_eval = self.eval_cleanup_expr(
                    states,
                    *iterable,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*iterable).span),
                );
                if !iterable_eval.continues {
                    return iterable_eval;
                }
                let body_eval = self.eval_cleanup_block(
                    iterable_eval.states.clone(),
                    *body,
                    reporter,
                    use_site,
                );
                CleanupEval::cont(merge_state_vec(Some(&iterable_eval.states), &body_eval.states).0)
            }
            hir::StmtKind::Expr { expr, .. } => self.eval_cleanup_expr(
                states,
                *expr,
                reporter,
                use_site.with_span(self.hir.expr(*expr).span),
            ),
        }
    }

    fn eval_cleanup_assign_target(
        &self,
        states: Vec<LocalState>,
        expr_id: hir::ExprId,
        reporter: Option<&mut Reporter>,
        use_site: UseSite,
    ) -> CleanupAssignTarget {
        let expr = self.hir.expr(expr_id);
        match &expr.kind {
            hir::ExprKind::Name(_) => {
                CleanupAssignTarget::cont(states, self.direct_local_for_expr(expr_id))
            }
            hir::ExprKind::Member { object, .. } => {
                let eval = self.eval_cleanup_expr(
                    states,
                    *object,
                    reporter,
                    use_site.with_span(self.hir.expr(*object).span),
                );
                CleanupAssignTarget::from_eval(eval, None)
            }
            hir::ExprKind::Bracket { target, items } => {
                let mut reporter = reporter;
                let target_eval = self.eval_cleanup_expr(
                    states,
                    *target,
                    reporter.as_deref_mut(),
                    use_site.with_span(self.hir.expr(*target).span),
                );
                if !target_eval.continues {
                    return CleanupAssignTarget::from_eval(target_eval, None);
                }
                let items_eval =
                    self.eval_cleanup_exprs(target_eval.states, items, reporter, use_site);
                CleanupAssignTarget::from_eval(items_eval, None)
            }
            _ => {
                let eval = self.eval_cleanup_expr(
                    states,
                    expr_id,
                    reporter,
                    use_site.with_span(expr.span),
                );
                CleanupAssignTarget::from_eval(eval, None)
            }
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

#[derive(Clone, Debug)]
struct CleanupEval {
    states: Vec<LocalState>,
    continues: bool,
}

impl CleanupEval {
    fn cont(states: Vec<LocalState>) -> Self {
        Self {
            states,
            continues: true,
        }
    }

    fn stop(states: Vec<LocalState>) -> Self {
        Self {
            states,
            continues: false,
        }
    }
}

#[derive(Clone, Debug)]
struct CleanupAssignTarget {
    states: Vec<LocalState>,
    continues: bool,
    root_local: Option<MirLocalId>,
}

impl CleanupAssignTarget {
    fn cont(states: Vec<LocalState>, root_local: Option<MirLocalId>) -> Self {
        Self {
            states,
            continues: true,
            root_local,
        }
    }

    fn from_eval(eval: CleanupEval, root_local: Option<MirLocalId>) -> Self {
        Self {
            states: eval.states,
            continues: eval.continues,
            root_local,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UseSite {
    span: ql_span::Span,
    label: &'static str,
    note: Option<&'static str>,
}

impl UseSite {
    fn normal(span: ql_span::Span) -> Self {
        Self {
            span,
            label: "use here",
            note: None,
        }
    }

    fn closure_capture(span: ql_span::Span) -> Self {
        Self {
            span,
            label: "captured here by closure",
            note: None,
        }
    }

    fn move_closure_capture(span: ql_span::Span) -> Self {
        Self {
            span,
            label: "captured here by `move` closure",
            note: Some("move closures consume captured locals when the closure value is created"),
        }
    }

    fn deferred(span: ql_span::Span) -> Self {
        Self {
            span,
            label: "used here when deferred cleanup runs",
            note: Some("deferred cleanup executes on scope exit in LIFO order"),
        }
    }

    fn with_span(self, span: ql_span::Span) -> Self {
        Self { span, ..self }
    }
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

fn merge_cleanup_branches(left: CleanupEval, right: CleanupEval) -> CleanupEval {
    match (left.continues, right.continues) {
        (true, true) => CleanupEval::cont(merge_state_vec(Some(&left.states), &right.states).0),
        (true, false) => CleanupEval::cont(left.states),
        (false, true) => CleanupEval::cont(right.states),
        (false, false) => CleanupEval::stop(merge_state_vec(Some(&left.states), &right.states).0),
    }
}

fn merge_cleanup_branch_set(
    base_states: Vec<LocalState>,
    results: Vec<CleanupEval>,
) -> CleanupEval {
    let mut continuing: Option<Vec<LocalState>> = None;
    let mut terminated: Option<Vec<LocalState>> = None;

    if results.is_empty() {
        return CleanupEval::cont(base_states);
    }

    for result in results {
        if result.continues {
            continuing = Some(match continuing {
                Some(existing) => merge_state_vec(Some(&existing), &result.states).0,
                None => result.states,
            });
        } else {
            terminated = Some(match terminated {
                Some(existing) => merge_state_vec(Some(&existing), &result.states).0,
                None => result.states,
            });
        }
    }

    match (continuing, terminated) {
        (Some(states), _) => CleanupEval::cont(states),
        (None, Some(states)) => CleanupEval::stop(states),
        (None, None) => CleanupEval::cont(base_states),
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
        MoveReason::MoveClosureCapture => "captured here by `move` closure".to_owned(),
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
