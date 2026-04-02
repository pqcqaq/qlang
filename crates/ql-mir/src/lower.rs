use std::collections::{HashMap, HashSet};

use ql_ast::{BinaryOp, ReceiverKind};
use ql_hir::{
    self as hir, CallArg, ExprId, ExprKind, Function, ItemId, ItemKind, Param, PatternId,
    PatternKind, StmtKind,
};
use ql_resolve::{ResolutionMap, ValueResolution};

use crate::{
    AggregateField, BasicBlock, BasicBlockId, BodyOwner, CallArgument, CleanupAction, CleanupKind,
    ClosureCapture, ClosureDecl, Constant, LocalDecl, LocalId, LocalKind, LocalOrigin,
    MatchArmTarget, MirBody, MirModule, MirScope, Operand, Place, ProjectionElem, Rvalue, ScopeId,
    ScopeKind, Statement, StatementKind, Terminator, TerminatorKind,
};

pub fn lower_module(hir: &hir::Module, resolution: &ResolutionMap) -> MirModule {
    let mut mir = MirModule::default();

    for &item_id in &hir.items {
        match &hir.item(item_id).kind {
            ItemKind::Function(function) => {
                if function.body.is_some() {
                    lower_function_body(
                        &mut mir,
                        hir,
                        resolution,
                        BodyOwner::Item(item_id),
                        function,
                    );
                }
            }
            ItemKind::Trait(trait_item) => {
                for (index, method) in trait_item.methods.iter().enumerate() {
                    if method.body.is_some() {
                        lower_function_body(
                            &mut mir,
                            hir,
                            resolution,
                            BodyOwner::TraitMethod {
                                item: item_id,
                                index,
                            },
                            method,
                        );
                    }
                }
            }
            ItemKind::Impl(impl_item) => {
                for (index, method) in impl_item.methods.iter().enumerate() {
                    if method.body.is_some() {
                        lower_function_body(
                            &mut mir,
                            hir,
                            resolution,
                            BodyOwner::ImplMethod {
                                item: item_id,
                                index,
                            },
                            method,
                        );
                    }
                }
            }
            ItemKind::Extend(extend_item) => {
                for (index, method) in extend_item.methods.iter().enumerate() {
                    if method.body.is_some() {
                        lower_function_body(
                            &mut mir,
                            hir,
                            resolution,
                            BodyOwner::ExtendMethod {
                                item: item_id,
                                index,
                            },
                            method,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    mir
}

fn lower_function_body(
    mir: &mut MirModule,
    hir: &hir::Module,
    resolution: &ResolutionMap,
    owner: BodyOwner,
    function: &Function,
) {
    let body = BodyBuilder::new(hir, resolution, owner, function).lower();
    mir.alloc_body(owner, body);
}

struct LoopFrame {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
    exit_scope_parent: Option<ScopeId>,
}

struct BodyBuilder<'a> {
    hir: &'a hir::Module,
    resolution: &'a ResolutionMap,
    function: &'a Function,
    body: MirBody,
    local_map: HashMap<hir::LocalId, LocalId>,
    param_locals: Vec<Option<LocalId>>,
    self_local: Option<LocalId>,
    temp_counter: usize,
    loop_stack: Vec<LoopFrame>,
}

impl<'a> BodyBuilder<'a> {
    fn new(
        hir: &'a hir::Module,
        resolution: &'a ResolutionMap,
        owner: BodyOwner,
        function: &'a Function,
    ) -> Self {
        let placeholder_block = BasicBlockId::from_index(0);
        let placeholder_scope = ScopeId::from_index(0);
        let placeholder_local = LocalId::from_index(0);

        Self {
            hir,
            resolution,
            function,
            body: MirBody {
                owner,
                name: function.name.clone(),
                span: function.span,
                entry: placeholder_block,
                return_block: placeholder_block,
                return_local: placeholder_local,
                root_scope: placeholder_scope,
                local_data: Vec::new(),
                block_data: Vec::new(),
                statement_data: Vec::new(),
                scope_data: Vec::new(),
                cleanup_data: Vec::new(),
                closure_data: Vec::new(),
            },
            local_map: HashMap::new(),
            param_locals: vec![None; function.params.len()],
            self_local: None,
            temp_counter: 0,
            loop_stack: Vec::new(),
        }
    }

    fn lower(mut self) -> MirBody {
        let root_scope = self.alloc_scope(self.function.span, ScopeKind::Function, None);
        let entry = self.new_block(self.function.span);
        let return_block = self.new_block(self.function.span);
        let return_local = self.body.alloc_local(LocalDecl {
            name: "$return".to_owned(),
            span: self.function.span,
            mutable: false,
            kind: LocalKind::Return,
            origin: LocalOrigin::ReturnSlot,
            scope: root_scope,
        });

        self.body.root_scope = root_scope;
        self.body.entry = entry;
        self.body.return_block = return_block;
        self.body.return_local = return_local;
        self.set_terminator(return_block, self.function.span, TerminatorKind::Return);

        self.lower_params(entry, root_scope);

        let body_block = self
            .function
            .body
            .expect("only body-bearing functions should reach MIR lowering");
        self.lower_scoped_block(
            body_block,
            entry,
            root_scope,
            Some(Place::local(return_local)),
            return_block,
        );

        self.body
    }

    fn lower_params(&mut self, entry: BasicBlockId, root_scope: ScopeId) {
        for (index, param) in self.function.params.iter().enumerate() {
            match param {
                Param::Regular(param) => {
                    let local = self.alloc_local_in_scope(
                        root_scope,
                        param.name.clone(),
                        param.name_span,
                        false,
                        LocalKind::Param,
                        LocalOrigin::Param { index },
                    );
                    self.param_locals[index] = Some(local);
                    self.push_statement(
                        entry,
                        param.name_span,
                        StatementKind::StorageLive { local },
                    );
                }
                Param::Receiver(receiver) => {
                    let local = self.alloc_local_in_scope(
                        root_scope,
                        "self".to_owned(),
                        receiver.span,
                        matches!(receiver.kind, ReceiverKind::Mutable),
                        LocalKind::Param,
                        LocalOrigin::Receiver,
                    );
                    self.self_local = Some(local);
                    self.param_locals[index] = Some(local);
                    self.push_statement(entry, receiver.span, StatementKind::StorageLive { local });
                }
            }
        }
    }

    fn lower_scoped_block(
        &mut self,
        block_id: hir::BlockId,
        entry: BasicBlockId,
        scope: ScopeId,
        tail_target: Option<Place>,
        next_target: BasicBlockId,
    ) {
        let block = self.hir.block(block_id);
        let mut current = Some(entry);

        for &stmt_id in &block.statements {
            let Some(block_id) = current else {
                break;
            };
            current = self.lower_stmt(stmt_id, block_id, scope);
        }

        let Some(current) = current else {
            return;
        };

        if let Some(expr_id) = block.tail {
            if let Some(place) = tail_target {
                self.lower_expr_into_target(expr_id, current, scope, place, next_target);
                return;
            }

            let final_block = self.lower_expr_statement(expr_id, current, scope);
            let final_block =
                self.emit_scope_exit(final_block, scope, self.body.scope(scope).parent);
            self.set_terminator(
                final_block,
                block.span,
                TerminatorKind::Goto {
                    target: next_target,
                },
            );
            return;
        }

        if let Some(place) = tail_target {
            self.push_statement(
                current,
                block.span,
                StatementKind::Assign {
                    place,
                    value: Rvalue::Use(Operand::Constant(Constant::Void)),
                },
            );
        }

        let current = self.emit_scope_exit(current, scope, self.body.scope(scope).parent);
        self.set_terminator(
            current,
            block.span,
            TerminatorKind::Goto {
                target: next_target,
            },
        );
    }

    fn lower_stmt(
        &mut self,
        stmt_id: hir::StmtId,
        current: BasicBlockId,
        scope: ScopeId,
    ) -> Option<BasicBlockId> {
        let stmt = self.hir.stmt(stmt_id);

        match &stmt.kind {
            StmtKind::Let {
                mutable,
                pattern,
                value,
            } => {
                let (current, value) = self.lower_expr_to_operand(*value, current, scope);
                self.emit_pattern_binding(current, *pattern, scope, *mutable, value);
                Some(current)
            }
            StmtKind::Return(expr) => {
                let current = if let Some(expr) = expr {
                    let (current, value) = self.lower_expr_to_operand(*expr, current, scope);
                    self.push_statement(
                        current,
                        stmt.span,
                        StatementKind::Assign {
                            place: Place::local(self.body.return_local),
                            value: Rvalue::Use(value),
                        },
                    );
                    current
                } else {
                    self.push_statement(
                        current,
                        stmt.span,
                        StatementKind::Assign {
                            place: Place::local(self.body.return_local),
                            value: Rvalue::Use(Operand::Constant(Constant::Void)),
                        },
                    );
                    current
                };

                let current = self.emit_scope_exit(current, scope, None);
                self.set_terminator(
                    current,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: self.body.return_block,
                    },
                );
                None
            }
            StmtKind::Defer(expr) => {
                let cleanup = self.body.alloc_cleanup(CleanupAction {
                    span: stmt.span,
                    scope,
                    kind: CleanupKind::Defer { expr: *expr },
                });
                self.body.scope_mut(scope).cleanups.push(cleanup);
                self.push_statement(
                    current,
                    stmt.span,
                    StatementKind::RegisterCleanup { cleanup },
                );
                Some(current)
            }
            StmtKind::Break => {
                let frame = self.loop_stack.last()?;
                let break_target = frame.break_target;
                let exit_scope_parent = frame.exit_scope_parent;
                let current = self.emit_scope_exit(current, scope, exit_scope_parent);
                self.set_terminator(
                    current,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: break_target,
                    },
                );
                None
            }
            StmtKind::Continue => {
                let frame = self.loop_stack.last()?;
                let continue_target = frame.continue_target;
                let exit_scope_parent = frame.exit_scope_parent;
                let current = self.emit_scope_exit(current, scope, exit_scope_parent);
                self.set_terminator(
                    current,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: continue_target,
                    },
                );
                None
            }
            StmtKind::While { condition, body } => {
                let header = self.new_block(stmt.span);
                let body_entry = self.new_block(self.hir.block(*body).span);
                let exit = self.new_block(stmt.span);
                self.set_terminator(current, stmt.span, TerminatorKind::Goto { target: header });

                let (condition_block, condition_value) =
                    self.lower_expr_to_operand(*condition, header, scope);
                self.set_terminator(
                    condition_block,
                    stmt.span,
                    TerminatorKind::Branch {
                        condition: condition_value,
                        then_target: body_entry,
                        else_target: exit,
                    },
                );

                let loop_scope =
                    self.alloc_scope(self.hir.block(*body).span, ScopeKind::Block, Some(scope));
                self.loop_stack.push(LoopFrame {
                    break_target: exit,
                    continue_target: header,
                    exit_scope_parent: Some(scope),
                });
                self.lower_scoped_block(*body, body_entry, loop_scope, None, header);
                self.loop_stack.pop();

                Some(exit)
            }
            StmtKind::Loop { body } => {
                let body_entry = self.new_block(self.hir.block(*body).span);
                let exit = self.new_block(stmt.span);
                self.set_terminator(
                    current,
                    stmt.span,
                    TerminatorKind::Goto { target: body_entry },
                );

                let loop_scope =
                    self.alloc_scope(self.hir.block(*body).span, ScopeKind::Block, Some(scope));
                self.loop_stack.push(LoopFrame {
                    break_target: exit,
                    continue_target: body_entry,
                    exit_scope_parent: Some(scope),
                });
                self.lower_scoped_block(*body, body_entry, loop_scope, None, body_entry);
                self.loop_stack.pop();

                Some(exit)
            }
            StmtKind::For {
                is_await,
                pattern,
                iterable,
                body,
            } => {
                let (current, iterable) = self.lower_expr_to_operand(*iterable, current, scope);
                let header = self.new_block(stmt.span);
                let body_entry = self.new_block(self.hir.block(*body).span);
                let exit = self.new_block(stmt.span);
                self.set_terminator(current, stmt.span, TerminatorKind::Goto { target: header });

                let loop_scope =
                    self.alloc_scope(self.hir.block(*body).span, ScopeKind::ForLoop, Some(scope));
                let item_local = self.alloc_temp(loop_scope, stmt.span);
                self.set_terminator(
                    header,
                    stmt.span,
                    TerminatorKind::ForLoop {
                        iterable,
                        item_local,
                        is_await: *is_await,
                        body_target: body_entry,
                        exit_target: exit,
                    },
                );

                self.push_statement(
                    body_entry,
                    stmt.span,
                    StatementKind::StorageLive { local: item_local },
                );
                self.emit_pattern_binding(
                    body_entry,
                    *pattern,
                    loop_scope,
                    false,
                    Operand::Place(Place::local(item_local)),
                );

                self.loop_stack.push(LoopFrame {
                    break_target: exit,
                    continue_target: header,
                    exit_scope_parent: Some(scope),
                });
                self.lower_scoped_block(*body, body_entry, loop_scope, None, header);
                self.loop_stack.pop();

                Some(exit)
            }
            StmtKind::Expr { expr, .. } => Some(self.lower_expr_statement(*expr, current, scope)),
        }
    }

    fn lower_expr_statement(
        &mut self,
        expr_id: ExprId,
        current: BasicBlockId,
        scope: ScopeId,
    ) -> BasicBlockId {
        let expr = self.hir.expr(expr_id);

        match &expr.kind {
            ExprKind::Call { callee, args } => {
                let (current, value) = self.lower_call_rvalue(*callee, args, current, scope);
                self.push_statement(current, expr.span, StatementKind::Eval { value });
                current
            }
            ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let (current, place) = self.lower_expr_to_place(*left, current, scope);
                let (current, value) = self.lower_expr_to_operand(*right, current, scope);
                self.push_statement(
                    current,
                    expr.span,
                    StatementKind::Assign {
                        place,
                        value: Rvalue::Use(value),
                    },
                );
                current
            }
            _ => {
                let (current, _) = self.lower_expr_to_operand(expr_id, current, scope);
                current
            }
        }
    }

    fn lower_expr_to_operand(
        &mut self,
        expr_id: ExprId,
        current: BasicBlockId,
        scope: ScopeId,
    ) -> (BasicBlockId, Operand) {
        let expr = self.hir.expr(expr_id);

        match &expr.kind {
            ExprKind::Name(name) => (current, self.lower_name_operand(expr_id, name, scope)),
            ExprKind::Integer(value) => {
                (current, Operand::Constant(Constant::Integer(value.clone())))
            }
            ExprKind::String { value, is_format } => (
                current,
                Operand::Constant(Constant::String {
                    value: value.clone(),
                    is_format: *is_format,
                }),
            ),
            ExprKind::Bool(value) => (current, Operand::Constant(Constant::Bool(*value))),
            ExprKind::NoneLiteral => (current, Operand::Constant(Constant::None)),
            ExprKind::Member { .. } | ExprKind::Bracket { .. } => {
                let (current, place) = self.lower_expr_to_place(expr_id, current, scope);
                (current, Operand::Place(place))
            }
            ExprKind::Tuple(items) => {
                let (current, items) = self.lower_operands(items, current, scope);
                self.materialize_rvalue(current, scope, expr.span, Rvalue::Tuple(items))
            }
            ExprKind::Array(items) => {
                let (current, items) = self.lower_operands(items, current, scope);
                self.materialize_rvalue(current, scope, expr.span, Rvalue::Array(items))
            }
            ExprKind::Call { callee, args } => {
                let (current, value) = self.lower_call_rvalue(*callee, args, current, scope);
                self.materialize_rvalue(current, scope, expr.span, value)
            }
            ExprKind::StructLiteral { path, fields } => {
                let mut current = current;
                let mut lowered = Vec::with_capacity(fields.len());
                for field in fields {
                    let (next, value) = self.lower_expr_to_operand(field.value, current, scope);
                    current = next;
                    lowered.push(AggregateField {
                        name: field.name.clone(),
                        value,
                    });
                }
                self.materialize_rvalue(
                    current,
                    scope,
                    expr.span,
                    Rvalue::AggregateStruct {
                        path: path.clone(),
                        fields: lowered,
                    },
                )
            }
            ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let (current, place) = self.lower_expr_to_place(*left, current, scope);
                let (current, value) = self.lower_expr_to_operand(*right, current, scope);
                self.push_statement(
                    current,
                    expr.span,
                    StatementKind::Assign {
                        place,
                        value: Rvalue::Use(value.clone()),
                    },
                );
                let (current, local) = self.materialize_operand(current, scope, expr.span, value);
                (current, Operand::Place(Place::local(local)))
            }
            ExprKind::Binary { left, op, right } => {
                let (current, left) = self.lower_expr_to_operand(*left, current, scope);
                let (current, right) = self.lower_expr_to_operand(*right, current, scope);
                self.materialize_rvalue(
                    current,
                    scope,
                    expr.span,
                    Rvalue::Binary {
                        left,
                        op: *op,
                        right,
                    },
                )
            }
            ExprKind::Unary { op, expr: inner } => {
                let (current, operand) = self.lower_expr_to_operand(*inner, current, scope);
                self.materialize_rvalue(
                    current,
                    scope,
                    expr.span,
                    Rvalue::Unary { op: *op, operand },
                )
            }
            ExprKind::Closure {
                is_move,
                params,
                body,
            } => {
                let captures = self.collect_closure_captures(*body);
                let params = params
                    .iter()
                    .map(|local_id| self.hir.local(*local_id).name.clone())
                    .collect();
                let closure = self.body.alloc_closure(ClosureDecl {
                    span: expr.span,
                    is_move: *is_move,
                    params,
                    captures,
                    body: *body,
                });
                self.materialize_rvalue(current, scope, expr.span, Rvalue::Closure { closure })
            }
            ExprKind::Question(inner) => {
                let (current, operand) = self.lower_expr_to_operand(*inner, current, scope);
                self.materialize_rvalue(current, scope, expr.span, Rvalue::Question(operand))
            }
            ExprKind::Block(_)
            | ExprKind::Unsafe(_)
            | ExprKind::If { .. }
            | ExprKind::Match { .. } => {
                let target = self.alloc_temp(scope, expr.span);
                self.push_statement(
                    current,
                    expr.span,
                    StatementKind::StorageLive { local: target },
                );
                let join = self.new_block(expr.span);
                self.lower_expr_into_target(expr_id, current, scope, Place::local(target), join);
                (join, Operand::Place(Place::local(target)))
            }
        }
    }

    fn lower_expr_into_target(
        &mut self,
        expr_id: ExprId,
        entry: BasicBlockId,
        scope: ScopeId,
        target: Place,
        join: BasicBlockId,
    ) {
        let expr = self.hir.expr(expr_id);

        match &expr.kind {
            ExprKind::Block(block_id) => {
                let child = self.alloc_scope(
                    self.hir.block(*block_id).span,
                    ScopeKind::Block,
                    Some(scope),
                );
                self.lower_scoped_block(*block_id, entry, child, Some(target), join);
            }
            ExprKind::Unsafe(block_id) => {
                let child = self.alloc_scope(
                    self.hir.block(*block_id).span,
                    ScopeKind::UnsafeBlock,
                    Some(scope),
                );
                self.lower_scoped_block(*block_id, entry, child, Some(target), join);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if else_branch.is_none() {
                    self.push_statement(
                        entry,
                        expr.span,
                        StatementKind::Assign {
                            place: target.clone(),
                            value: Rvalue::Use(Operand::Constant(Constant::Void)),
                        },
                    );
                }

                let (condition_block, condition_value) =
                    self.lower_expr_to_operand(*condition, entry, scope);
                let then_entry = self.new_block(self.hir.block(*then_branch).span);
                let else_entry = if else_branch.is_some() {
                    self.new_block(expr.span)
                } else {
                    join
                };
                self.set_terminator(
                    condition_block,
                    expr.span,
                    TerminatorKind::Branch {
                        condition: condition_value,
                        then_target: then_entry,
                        else_target: else_entry,
                    },
                );

                let then_scope = self.alloc_scope(
                    self.hir.block(*then_branch).span,
                    ScopeKind::Block,
                    Some(scope),
                );
                self.lower_scoped_block(
                    *then_branch,
                    then_entry,
                    then_scope,
                    Some(target.clone()),
                    join,
                );

                if let Some(else_expr) = else_branch {
                    self.lower_expr_into_target(*else_expr, else_entry, scope, target, join);
                }
            }
            ExprKind::Match { value, arms } => {
                let (dispatch, scrutinee) = self.lower_expr_to_operand(*value, entry, scope);
                let (dispatch, scrutinee) =
                    self.materialize_operand(dispatch, scope, expr.span, scrutinee);

                let mut lowered_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    let arm_entry = self.new_block(self.hir.expr(arm.body).span);
                    let arm_scope = self.alloc_scope(
                        self.hir.expr(arm.body).span,
                        ScopeKind::MatchArm,
                        Some(scope),
                    );
                    self.emit_pattern_binding(
                        arm_entry,
                        arm.pattern,
                        arm_scope,
                        false,
                        Operand::Place(Place::local(scrutinee)),
                    );
                    self.lower_expr_into_target(
                        arm.body,
                        arm_entry,
                        arm_scope,
                        target.clone(),
                        join,
                    );
                    lowered_arms.push(MatchArmTarget {
                        pattern: arm.pattern,
                        guard: arm.guard,
                        target: arm_entry,
                    });
                }

                self.set_terminator(
                    dispatch,
                    expr.span,
                    TerminatorKind::Match {
                        scrutinee: Operand::Place(Place::local(scrutinee)),
                        arms: lowered_arms,
                        else_target: join,
                    },
                );
            }
            _ => {
                let (current, value) = self.lower_expr_to_operand(expr_id, entry, scope);
                self.push_statement(
                    current,
                    expr.span,
                    StatementKind::Assign {
                        place: target,
                        value: Rvalue::Use(value),
                    },
                );
                self.set_terminator(current, expr.span, TerminatorKind::Goto { target: join });
            }
        }
    }

    fn lower_expr_to_place(
        &mut self,
        expr_id: ExprId,
        current: BasicBlockId,
        scope: ScopeId,
    ) -> (BasicBlockId, Place) {
        let expr = self.hir.expr(expr_id);

        match &expr.kind {
            ExprKind::Name(name) => match self.lower_name_operand(expr_id, name, scope) {
                Operand::Place(place) => (current, place),
                operand => {
                    let (current, local) =
                        self.materialize_operand(current, scope, expr.span, operand);
                    (current, Place::local(local))
                }
            },
            ExprKind::Member { object, field, .. } => {
                let (current, mut place) = self.lower_expr_to_place(*object, current, scope);
                place.projections.push(ProjectionElem::Field(field.clone()));
                (current, place)
            }
            ExprKind::Bracket { target, items } => {
                let (mut current, mut place) = self.lower_expr_to_place(*target, current, scope);
                for item in items {
                    let (next, value) = self.lower_expr_to_operand(*item, current, scope);
                    current = next;
                    place
                        .projections
                        .push(ProjectionElem::Index(Box::new(value)));
                }
                (current, place)
            }
            _ => {
                let (current, operand) = self.lower_expr_to_operand(expr_id, current, scope);
                let (current, local) = self.materialize_operand(current, scope, expr.span, operand);
                (current, Place::local(local))
            }
        }
    }

    fn lower_call_rvalue(
        &mut self,
        callee: ExprId,
        args: &[CallArg],
        current: BasicBlockId,
        scope: ScopeId,
    ) -> (BasicBlockId, Rvalue) {
        let (mut current, callee) = self.lower_expr_to_operand(callee, current, scope);
        let mut lowered = Vec::with_capacity(args.len());

        for arg in args {
            match arg {
                CallArg::Positional(expr_id) => {
                    let (next, value) = self.lower_expr_to_operand(*expr_id, current, scope);
                    current = next;
                    lowered.push(CallArgument { name: None, value });
                }
                CallArg::Named { name, value, .. } => {
                    let (next, value) = self.lower_expr_to_operand(*value, current, scope);
                    current = next;
                    lowered.push(CallArgument {
                        name: Some(name.clone()),
                        value,
                    });
                }
            }
        }

        (
            current,
            Rvalue::Call {
                callee,
                args: lowered,
            },
        )
    }

    fn lower_operands(
        &mut self,
        exprs: &[ExprId],
        mut current: BasicBlockId,
        scope: ScopeId,
    ) -> (BasicBlockId, Vec<Operand>) {
        let mut operands = Vec::with_capacity(exprs.len());
        for expr in exprs {
            let (next, operand) = self.lower_expr_to_operand(*expr, current, scope);
            current = next;
            operands.push(operand);
        }
        (current, operands)
    }

    fn lower_name_operand(&mut self, expr_id: ExprId, name: &str, scope: ScopeId) -> Operand {
        match self.resolution.expr_resolution(expr_id) {
            Some(ValueResolution::Local(local_id)) => {
                let local = self.ensure_binding_local(*local_id, scope, false);
                Operand::Place(Place::local(local))
            }
            Some(ValueResolution::Param(binding)) => self
                .param_locals
                .get(binding.index)
                .and_then(|local| *local)
                .map(|local| Operand::Place(Place::local(local)))
                .unwrap_or_else(|| Operand::Constant(Constant::UnresolvedName(name.to_owned()))),
            Some(ValueResolution::SelfValue) => self
                .self_local
                .map(|local| Operand::Place(Place::local(local)))
                .unwrap_or_else(|| Operand::Constant(Constant::UnresolvedName("self".to_owned()))),
            Some(ValueResolution::Function(function)) => Operand::Constant(Constant::Function {
                function: *function,
                name: self.hir.function(*function).name.clone(),
            }),
            Some(ValueResolution::Item(item)) => Operand::Constant(Constant::Item {
                item: *item,
                name: self.item_name(*item),
            }),
            Some(ValueResolution::Import(binding)) => {
                Operand::Constant(Constant::Import(binding.path.clone()))
            }
            None => Operand::Constant(Constant::UnresolvedName(name.to_owned())),
        }
    }

    fn local_for_resolution(&self, resolution: &ValueResolution) -> Option<LocalId> {
        match resolution {
            ValueResolution::Local(local_id) => self.local_map.get(local_id).copied(),
            ValueResolution::Param(binding) => self
                .param_locals
                .get(binding.index)
                .and_then(|local| *local),
            ValueResolution::SelfValue => self.self_local,
            ValueResolution::Function(_) => None,
            ValueResolution::Item(_) | ValueResolution::Import(_) => None,
        }
    }

    fn direct_local_for_expr(&self, expr_id: ExprId) -> Option<LocalId> {
        match &self.hir.expr(expr_id).kind {
            ExprKind::Name(_) => self
                .resolution
                .expr_resolution(expr_id)
                .and_then(|resolution| self.local_for_resolution(resolution)),
            _ => None,
        }
    }

    fn collect_closure_captures(&self, expr_id: ExprId) -> Vec<ClosureCapture> {
        let mut captures = Vec::new();
        let mut seen = HashSet::new();
        self.collect_expr_captures(expr_id, &mut captures, &mut seen);
        captures
    }

    fn collect_expr_captures(
        &self,
        expr_id: ExprId,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<LocalId>,
    ) {
        if let Some(local) = self.direct_local_for_expr(expr_id)
            && seen.insert(local)
        {
            captures.push(ClosureCapture {
                local,
                span: self.hir.expr(expr_id).span,
            });
        }

        match &self.hir.expr(expr_id).kind {
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for item in items {
                    self.collect_expr_captures(*item, captures, seen);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.collect_block_captures(*block_id, captures, seen);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr_captures(*condition, captures, seen);
                self.collect_block_captures(*then_branch, captures, seen);
                if let Some(else_branch) = else_branch {
                    self.collect_expr_captures(*else_branch, captures, seen);
                }
            }
            ExprKind::Match { value, arms } => {
                self.collect_expr_captures(*value, captures, seen);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.collect_expr_captures(guard, captures, seen);
                    }
                    self.collect_expr_captures(arm.body, captures, seen);
                }
            }
            ExprKind::Closure { body, .. } => {
                self.collect_expr_captures(*body, captures, seen);
            }
            ExprKind::Call { callee, args } => {
                self.collect_expr_captures(*callee, captures, seen);
                for arg in args {
                    match arg {
                        CallArg::Positional(value) => {
                            self.collect_expr_captures(*value, captures, seen)
                        }
                        CallArg::Named { value, .. } => {
                            self.collect_expr_captures(*value, captures, seen)
                        }
                    }
                }
            }
            ExprKind::Member { object, .. } => {
                self.collect_expr_captures(*object, captures, seen);
            }
            ExprKind::Bracket { target, items } => {
                self.collect_expr_captures(*target, captures, seen);
                for item in items {
                    self.collect_expr_captures(*item, captures, seen);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.collect_expr_captures(field.value, captures, seen);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_expr_captures(*left, captures, seen);
                self.collect_expr_captures(*right, captures, seen);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.collect_expr_captures(*expr, captures, seen);
            }
        }
    }

    fn collect_block_captures(
        &self,
        block_id: hir::BlockId,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<LocalId>,
    ) {
        let block = self.hir.block(block_id);
        for stmt_id in &block.statements {
            self.collect_stmt_captures(*stmt_id, captures, seen);
        }
        if let Some(tail) = block.tail {
            self.collect_expr_captures(tail, captures, seen);
        }
    }

    fn collect_stmt_captures(
        &self,
        stmt_id: hir::StmtId,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<LocalId>,
    ) {
        match &self.hir.stmt(stmt_id).kind {
            StmtKind::Let { value, .. } => self.collect_expr_captures(*value, captures, seen),
            StmtKind::Return(Some(expr)) | StmtKind::Defer(expr) => {
                self.collect_expr_captures(*expr, captures, seen);
            }
            StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
            StmtKind::While { condition, body } => {
                self.collect_expr_captures(*condition, captures, seen);
                self.collect_block_captures(*body, captures, seen);
            }
            StmtKind::Loop { body } => self.collect_block_captures(*body, captures, seen),
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr_captures(*iterable, captures, seen);
                self.collect_block_captures(*body, captures, seen);
            }
            StmtKind::Expr { expr, .. } => self.collect_expr_captures(*expr, captures, seen),
        }
    }

    fn emit_pattern_binding(
        &mut self,
        current: BasicBlockId,
        pattern: PatternId,
        scope: ScopeId,
        mutable: bool,
        source: Operand,
    ) {
        for hir_local in self.collect_binding_locals(pattern) {
            let local = self.ensure_binding_local(hir_local, scope, mutable);
            self.push_statement(
                current,
                self.hir.local(hir_local).span,
                StatementKind::StorageLive { local },
            );
        }

        self.push_statement(
            current,
            self.hir.pattern(pattern).span,
            StatementKind::BindPattern {
                pattern,
                source,
                mutable,
            },
        );
    }

    fn collect_binding_locals(&self, pattern: PatternId) -> Vec<hir::LocalId> {
        let mut locals = Vec::new();
        self.collect_binding_locals_recursive(pattern, &mut locals);
        locals
    }

    fn collect_binding_locals_recursive(&self, pattern: PatternId, locals: &mut Vec<hir::LocalId>) {
        match &self.hir.pattern(pattern).kind {
            PatternKind::Binding(local) => locals.push(*local),
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for item in items {
                    self.collect_binding_locals_recursive(*item, locals);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    self.collect_binding_locals_recursive(field.pattern, locals);
                }
            }
            PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn ensure_binding_local(
        &mut self,
        hir_local: hir::LocalId,
        scope: ScopeId,
        mutable: bool,
    ) -> LocalId {
        if let Some(local) = self.local_map.get(&hir_local) {
            return *local;
        }

        let local = self.alloc_local_in_scope(
            scope,
            self.hir.local(hir_local).name.clone(),
            self.hir.local(hir_local).span,
            mutable,
            LocalKind::Binding,
            LocalOrigin::Binding(hir_local),
        );
        self.local_map.insert(hir_local, local);
        local
    }

    fn emit_scope_exit(
        &mut self,
        current: BasicBlockId,
        scope: ScopeId,
        stop_before: Option<ScopeId>,
    ) -> BasicBlockId {
        let mut cursor = Some(scope);
        while let Some(scope_id) = cursor {
            if Some(scope_id) == stop_before {
                break;
            }

            let scope = self.body.scope(scope_id).clone();
            for cleanup in scope.cleanups.iter().rev() {
                self.push_statement(
                    current,
                    self.body.cleanup(*cleanup).span,
                    StatementKind::RunCleanup { cleanup: *cleanup },
                );
            }
            for local in scope.locals.iter().rev() {
                self.push_statement(
                    current,
                    self.body.local(*local).span,
                    StatementKind::StorageDead { local: *local },
                );
            }
            cursor = scope.parent;
        }
        current
    }

    fn materialize_rvalue(
        &mut self,
        current: BasicBlockId,
        scope: ScopeId,
        span: ql_span::Span,
        value: Rvalue,
    ) -> (BasicBlockId, Operand) {
        let local = self.alloc_temp(scope, span);
        self.push_statement(current, span, StatementKind::StorageLive { local });
        self.push_statement(
            current,
            span,
            StatementKind::Assign {
                place: Place::local(local),
                value,
            },
        );
        (current, Operand::Place(Place::local(local)))
    }

    fn materialize_operand(
        &mut self,
        current: BasicBlockId,
        scope: ScopeId,
        span: ql_span::Span,
        operand: Operand,
    ) -> (BasicBlockId, LocalId) {
        let local = self.alloc_temp(scope, span);
        self.push_statement(current, span, StatementKind::StorageLive { local });
        self.push_statement(
            current,
            span,
            StatementKind::Assign {
                place: Place::local(local),
                value: Rvalue::Use(operand),
            },
        );
        (current, local)
    }

    fn alloc_temp(&mut self, scope: ScopeId, span: ql_span::Span) -> LocalId {
        let ordinal = self.temp_counter;
        self.temp_counter += 1;
        self.alloc_local_in_scope(
            scope,
            format!("_t{ordinal}"),
            span,
            false,
            LocalKind::Temp,
            LocalOrigin::Temp { ordinal },
        )
    }

    fn alloc_local_in_scope(
        &mut self,
        scope: ScopeId,
        name: String,
        span: ql_span::Span,
        mutable: bool,
        kind: LocalKind,
        origin: LocalOrigin,
    ) -> LocalId {
        let local = self.body.alloc_local(LocalDecl {
            name,
            span,
            mutable,
            kind,
            origin,
            scope,
        });
        self.body.scope_mut(scope).locals.push(local);
        local
    }

    fn alloc_scope(
        &mut self,
        span: ql_span::Span,
        kind: ScopeKind,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        self.body.alloc_scope(MirScope {
            span,
            kind,
            parent,
            locals: Vec::new(),
            cleanups: Vec::new(),
        })
    }

    fn new_block(&mut self, span: ql_span::Span) -> BasicBlockId {
        self.body.alloc_block(BasicBlock {
            span,
            statements: Vec::new(),
            terminator: Terminator {
                span,
                kind: TerminatorKind::Terminate,
            },
        })
    }

    fn push_statement(&mut self, block: BasicBlockId, span: ql_span::Span, kind: StatementKind) {
        let statement = self.body.alloc_statement(Statement { span, kind });
        self.body.block_mut(block).statements.push(statement);
    }

    fn set_terminator(&mut self, block: BasicBlockId, span: ql_span::Span, kind: TerminatorKind) {
        self.body.block_mut(block).terminator = Terminator { span, kind };
    }

    fn item_name(&self, item: ItemId) -> String {
        match &self.hir.item(item).kind {
            ItemKind::Function(function) => function.name.clone(),
            ItemKind::Const(global) | ItemKind::Static(global) => global.name.clone(),
            ItemKind::Struct(struct_item) => struct_item.name.clone(),
            ItemKind::Enum(enum_item) => enum_item.name.clone(),
            ItemKind::Trait(trait_item) => trait_item.name.clone(),
            ItemKind::Impl(_) => "impl".to_owned(),
            ItemKind::Extend(_) => "extend".to_owned(),
            ItemKind::TypeAlias(alias) => alias.name.clone(),
            ItemKind::ExternBlock(_) => "extern".to_owned(),
        }
    }
}
