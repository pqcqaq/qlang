use ql_hir::{
    BlockId, ExprId, ExprKind, Function, ItemKind, MatchArm, Module, StmtKind, StructLiteralField,
};
use ql_runtime::RuntimeCapability;
use ql_span::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRequirement {
    pub capability: RuntimeCapability,
    pub span: Span,
}

pub(crate) fn collect_runtime_requirements(
    source: &str,
    module: &Module,
) -> Vec<RuntimeRequirement> {
    let mut collector = RuntimeRequirementCollector {
        source,
        module,
        requirements: Vec::new(),
    };
    for &item_id in &module.items {
        collector.visit_item(item_id);
    }
    collector.finish()
}

struct RuntimeRequirementCollector<'a> {
    source: &'a str,
    module: &'a Module,
    requirements: Vec<RuntimeRequirement>,
}

impl<'a> RuntimeRequirementCollector<'a> {
    fn finish(mut self) -> Vec<RuntimeRequirement> {
        self.requirements.sort_by(|left, right| {
            left.span
                .start
                .cmp(&right.span.start)
                .then(left.span.end.cmp(&right.span.end))
                .then(
                    left.capability
                        .stable_name()
                        .cmp(right.capability.stable_name()),
                )
        });
        self.requirements.dedup();
        self.requirements
    }

    fn push(&mut self, capability: RuntimeCapability, span: Span) {
        self.requirements
            .push(RuntimeRequirement { capability, span });
    }

    fn visit_item(&mut self, item_id: ql_hir::ItemId) {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => self.visit_function(function),
            ItemKind::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    self.visit_function(method);
                }
            }
            ItemKind::Impl(impl_block) => {
                for method in &impl_block.methods {
                    self.visit_function(method);
                }
            }
            ItemKind::Extend(extend_block) => {
                for method in &extend_block.methods {
                    self.visit_function(method);
                }
            }
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    self.visit_function(function);
                }
            }
            ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::TypeAlias(_) => {}
        }
    }

    fn visit_function(&mut self, function: &Function) {
        if function.is_async && function.body.is_some() {
            self.push(RuntimeCapability::AsyncFunctionBodies, function.span);
        }
        if let Some(body) = function.body {
            self.visit_block(body);
        }
    }

    fn visit_block(&mut self, block_id: BlockId) {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                StmtKind::Let { value, .. } => self.visit_expr(*value),
                StmtKind::Return(Some(expr_id)) | StmtKind::Defer(expr_id) => {
                    self.visit_expr(*expr_id)
                }
                StmtKind::While { condition, body } => {
                    self.visit_expr(*condition);
                    self.visit_block(*body);
                }
                StmtKind::Loop { body } => self.visit_block(*body),
                StmtKind::For {
                    is_await,
                    iterable,
                    body,
                    ..
                } => {
                    if *is_await {
                        self.push(
                            RuntimeCapability::AsyncIteration,
                            self.for_await_operator_span(stmt.span),
                        );
                    }
                    self.visit_expr(*iterable);
                    self.visit_block(*body);
                }
                StmtKind::Expr { expr, .. } => self.visit_expr(*expr),
                StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
            }
        }
        if let Some(tail) = block.tail {
            self.visit_expr(tail);
        }
    }

    fn visit_expr(&mut self, expr_id: ExprId) {
        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.visit_expr(item);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => self.visit_block(*block_id),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_expr(*condition);
                self.visit_block(*then_branch);
                if let Some(else_branch) = else_branch {
                    self.visit_expr(*else_branch);
                }
            }
            ExprKind::Match { value, arms } => {
                self.visit_expr(*value);
                for MatchArm { guard, body, .. } in arms {
                    if let Some(guard) = guard {
                        self.visit_expr(*guard);
                    }
                    self.visit_expr(*body);
                }
            }
            ExprKind::Closure { body, .. } => self.visit_expr(*body),
            ExprKind::Call { callee, args } => {
                self.visit_expr(*callee);
                for arg in args {
                    match arg {
                        ql_hir::CallArg::Positional(expr_id) => self.visit_expr(*expr_id),
                        ql_hir::CallArg::Named { value, .. } => self.visit_expr(*value),
                    }
                }
            }
            ExprKind::Member { object, .. } => self.visit_expr(*object),
            ExprKind::Bracket { target, items } => {
                self.visit_expr(*target);
                for &item in items {
                    self.visit_expr(item);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for StructLiteralField { value, .. } in fields {
                    self.visit_expr(*value);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(*left);
                self.visit_expr(*right);
            }
            ExprKind::Unary { op, expr: inner } => {
                match op {
                    ql_ast::UnaryOp::Await => {
                        self.push(RuntimeCapability::TaskAwait, self.root_span(expr.span));
                    }
                    ql_ast::UnaryOp::Spawn => {
                        self.push(RuntimeCapability::TaskSpawn, self.root_span(expr.span));
                    }
                    ql_ast::UnaryOp::Neg => {}
                }
                self.visit_expr(*inner);
            }
            ExprKind::Question(inner) => self.visit_expr(*inner),
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
        }
    }

    fn root_span(&self, span: Span) -> Span {
        let Some(slice) = self.source.get(span.start..span.end) else {
            return span;
        };

        for (offset, ch) in slice.char_indices() {
            if matches!(ch, '.' | '[' | '{' | '(' | ' ' | '\t' | '\r' | '\n') {
                if offset == 0 {
                    return span;
                }
                return Span::new(span.start, span.start + offset);
            }
        }

        span
    }

    fn for_await_operator_span(&self, stmt_span: Span) -> Span {
        let fallback = self.root_span(stmt_span);
        let Some(stmt_text) = self.source.get(stmt_span.start..stmt_span.end) else {
            return fallback;
        };

        let mut offset = skip_whitespace_prefix(stmt_text, 0);
        let Some(rest) = stmt_text.get(offset..) else {
            return fallback;
        };
        if !rest.starts_with("for") {
            return fallback;
        }

        offset += "for".len();
        offset = skip_whitespace_prefix(stmt_text, offset);
        let Some(rest) = stmt_text.get(offset..) else {
            return fallback;
        };
        if !rest.starts_with("await") {
            return fallback;
        }

        let start = stmt_span.start + offset;
        Span::new(start, start + "await".len())
    }
}

fn skip_whitespace_prefix(text: &str, mut offset: usize) -> usize {
    while let Some(ch) = text.get(offset..).and_then(|rest| rest.chars().next()) {
        if !ch.is_whitespace() {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}
