use std::fmt::Write;

use ql_ast::{BinaryOp, UnaryOp};
use ql_hir::{self as hir, ExprId, PatternId};

use crate::{
    AggregateField, BodyOwner, CleanupId, MirModule, Operand, Place, ProjectionElem, Rvalue,
    ScopeKind, StatementKind, TerminatorKind,
};

pub fn render_module(mir: &MirModule, hir: &hir::Module) -> String {
    let mut output = String::new();

    for (index, body_id) in mir.bodies().iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_body(&mut output, mir, hir, *body_id);
    }

    output
}

fn render_body(output: &mut String, mir: &MirModule, hir: &hir::Module, body_id: crate::BodyId) {
    let body = mir.body(body_id);
    let _ = writeln!(
        output,
        "body {} {} ({})",
        body_id.index(),
        body.name,
        render_owner(body.owner)
    );
    let _ = writeln!(
        output,
        "  entry=bb{} return=bb{} return_local=l{} root_scope=s{}",
        body.entry.index(),
        body.return_block.index(),
        body.return_local.index(),
        body.root_scope.index()
    );

    output.push_str("  locals:\n");
    for (index, local) in body.locals().iter().enumerate() {
        let _ = writeln!(
            output,
            "    l{index} {} {:?} mutable={} scope=s{}",
            local.name,
            local.kind,
            local.mutable,
            local.scope.index()
        );
    }

    output.push_str("  scopes:\n");
    for (index, scope) in body.scopes().iter().enumerate() {
        let parent = scope
            .parent
            .map(|scope| format!("s{}", scope.index()))
            .unwrap_or_else(|| "-".to_owned());
        let _ = writeln!(
            output,
            "    s{index} {} parent={} locals=[{}] cleanups=[{}]",
            render_scope_kind(scope.kind),
            parent,
            scope
                .locals
                .iter()
                .map(|local| format!("l{}", local.index()))
                .collect::<Vec<_>>()
                .join(", "),
            scope
                .cleanups
                .iter()
                .map(|cleanup| format!("c{}", cleanup.index()))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    output.push_str("  cleanups:\n");
    for (index, cleanup) in body.cleanups().iter().enumerate() {
        let _ = writeln!(
            output,
            "    c{index} {} @ s{}",
            render_cleanup_kind(hir, &cleanup.kind),
            cleanup.scope.index()
        );
    }

    output.push_str("  closures:\n");
    for closure_id in body.closure_ids() {
        let closure = body.closure(closure_id);
        let _ = writeln!(
            output,
            "    cl{} {}{}({}) => {}",
            closure_id.index(),
            if closure.is_move { "move " } else { "" },
            render_closure_captures(body, &closure.captures),
            closure.params.join(", "),
            render_expr(hir, closure.body)
        );
    }

    output.push_str("  blocks:\n");
    for (index, block) in body.blocks().iter().enumerate() {
        let _ = writeln!(output, "    bb{index}:");
        for statement_id in &block.statements {
            let statement = body.statement(*statement_id);
            let _ = writeln!(
                output,
                "      {}",
                render_statement(hir, body, &statement.kind)
            );
        }
        let _ = writeln!(
            output,
            "      {}",
            render_terminator(hir, body, &block.terminator.kind)
        );
    }
}

fn render_owner(owner: BodyOwner) -> String {
    match owner {
        BodyOwner::Item(item) => format!("item#{}", item.index()),
        BodyOwner::TraitMethod { item, index } => format!("trait#{}.method[{index}]", item.index()),
        BodyOwner::ImplMethod { item, index } => format!("impl#{}.method[{index}]", item.index()),
        BodyOwner::ExtendMethod { item, index } => {
            format!("extend#{}.method[{index}]", item.index())
        }
    }
}

fn render_scope_kind(kind: ScopeKind) -> &'static str {
    match kind {
        ScopeKind::Function => "function",
        ScopeKind::Block => "block",
        ScopeKind::UnsafeBlock => "unsafe-block",
        ScopeKind::MatchArm => "match-arm",
        ScopeKind::ForLoop => "for-loop",
    }
}

fn render_statement(hir: &hir::Module, body: &crate::MirBody, statement: &StatementKind) -> String {
    match statement {
        StatementKind::Assign { place, value } => {
            format!(
                "assign {} = {}",
                render_place(body, place),
                render_rvalue(hir, body, value)
            )
        }
        StatementKind::BindPattern {
            pattern,
            source,
            mutable,
        } => format!(
            "bind_pattern {} <- {} mutable={mutable}",
            render_pattern(hir, *pattern),
            render_operand(body, source)
        ),
        StatementKind::Eval { value } => format!("eval {}", render_rvalue(hir, body, value)),
        StatementKind::StorageLive { local } => format!("storage_live l{}", local.index()),
        StatementKind::StorageDead { local } => format!("storage_dead l{}", local.index()),
        StatementKind::RegisterCleanup { cleanup } => {
            format!("register_cleanup {}", render_cleanup(*cleanup))
        }
        StatementKind::RunCleanup { cleanup } => {
            format!("run_cleanup {}", render_cleanup(*cleanup))
        }
    }
}

fn render_terminator(
    hir: &hir::Module,
    body: &crate::MirBody,
    terminator: &TerminatorKind,
) -> String {
    match terminator {
        TerminatorKind::Goto { target } => format!("goto bb{}", target.index()),
        TerminatorKind::Branch {
            condition,
            then_target,
            else_target,
        } => format!(
            "branch {} ? bb{} : bb{}",
            render_operand(body, condition),
            then_target.index(),
            else_target.index()
        ),
        TerminatorKind::Match {
            scrutinee,
            arms,
            else_target,
        } => {
            let arms = arms
                .iter()
                .map(|arm| {
                    let guard = arm
                        .guard
                        .map(|guard| format!(" if {}", render_expr(hir, guard)))
                        .unwrap_or_default();
                    format!(
                        "{}{} -> bb{}",
                        render_pattern(hir, arm.pattern),
                        guard,
                        arm.target.index()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "match {} [{}] else bb{}",
                render_operand(body, scrutinee),
                arms,
                else_target.index()
            )
        }
        TerminatorKind::ForLoop {
            iterable,
            item_local,
            is_await,
            body_target,
            exit_target,
        } => format!(
            "for{} {} item=l{} body=bb{} exit=bb{}",
            if *is_await { " await" } else { "" },
            render_operand(body, iterable),
            item_local.index(),
            body_target.index(),
            exit_target.index()
        ),
        TerminatorKind::Return => "return".to_owned(),
        TerminatorKind::Terminate => "terminate".to_owned(),
    }
}

fn render_cleanup(cleanup: CleanupId) -> String {
    format!("c{}", cleanup.index())
}

fn render_cleanup_kind(hir: &hir::Module, cleanup: &crate::CleanupKind) -> String {
    match cleanup {
        crate::CleanupKind::Defer { expr } => format!("defer {}", render_expr(hir, *expr)),
    }
}

fn render_rvalue(hir: &hir::Module, body: &crate::MirBody, value: &Rvalue) -> String {
    match value {
        Rvalue::Use(operand) => render_operand(body, operand),
        Rvalue::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(|item| render_operand(body, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| render_operand(body, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::Call { callee, args } => format!(
            "{}({})",
            render_operand(body, callee),
            args.iter()
                .map(|arg| match &arg.name {
                    Some(name) => format!("{name}: {}", render_operand(body, &arg.value)),
                    None => render_operand(body, &arg.value),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::Binary { left, op, right } => format!(
            "{} {} {}",
            render_operand(body, left),
            render_binary(*op),
            render_operand(body, right)
        ),
        Rvalue::Unary { op, operand } => {
            format!("{}{}", render_unary(*op), render_operand(body, operand))
        }
        Rvalue::AggregateStruct { path, fields } => {
            format!(
                "{} {{ {} }}",
                render_path(path),
                render_fields(body, fields)
            )
        }
        Rvalue::Closure { closure } => {
            let closure_decl = body.closure(*closure);
            format!(
                "closure cl{} {}{}({}) => {}",
                closure.index(),
                if closure_decl.is_move { "move " } else { "" },
                render_closure_captures(body, &closure_decl.captures),
                closure_decl.params.join(", "),
                render_expr(hir, closure_decl.body)
            )
        }
        Rvalue::Question(operand) => format!("{}?", render_operand(body, operand)),
        Rvalue::OpaqueExpr(expr) => format!("<opaque {}>", render_expr(hir, *expr)),
    }
}

fn render_fields(body: &crate::MirBody, fields: &[AggregateField]) -> String {
    fields
        .iter()
        .map(|field| format!("{}: {}", field.name, render_operand(body, &field.value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_closure_captures(body: &crate::MirBody, captures: &[crate::ClosureCapture]) -> String {
    if captures.is_empty() {
        return String::new();
    }

    format!(
        "[captures: {}] ",
        captures
            .iter()
            .map(|capture| {
                format!(
                    "{}@{}..{}",
                    body.local(capture.local).name,
                    capture.span.start,
                    capture.span.end
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_operand(body: &crate::MirBody, operand: &Operand) -> String {
    match operand {
        Operand::Place(place) => render_place(body, place),
        Operand::Constant(constant) => render_constant(constant),
    }
}

fn render_place(body: &crate::MirBody, place: &Place) -> String {
    let mut text = format!("l{}({})", place.base.index(), body.local(place.base).name);
    for projection in &place.projections {
        match projection {
            ProjectionElem::Field(field) => {
                text.push('.');
                text.push_str(field);
            }
            ProjectionElem::TupleIndex(index) => {
                let _ = write!(text, ".{index}");
            }
            ProjectionElem::Index(index) => {
                let _ = write!(text, "[{}]", render_operand(body, index));
            }
        }
    }
    text
}

fn render_expr(hir: &hir::Module, expr_id: ExprId) -> String {
    match &hir.expr(expr_id).kind {
        hir::ExprKind::Name(name) => name.clone(),
        hir::ExprKind::Integer(value) => value.clone(),
        hir::ExprKind::String { value, is_format } => {
            if *is_format {
                format!("f\"{value}\"")
            } else {
                format!("\"{value}\"")
            }
        }
        hir::ExprKind::Bool(value) => value.to_string(),
        hir::ExprKind::NoneLiteral => "none".to_owned(),
        hir::ExprKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(|expr| render_expr(hir, *expr))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::ExprKind::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|expr| render_expr(hir, *expr))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::ExprKind::Block(_) => "{ ... }".to_owned(),
        hir::ExprKind::Unsafe(_) => "unsafe { ... }".to_owned(),
        hir::ExprKind::If { .. } => "if ...".to_owned(),
        hir::ExprKind::Match { .. } => "match ...".to_owned(),
        hir::ExprKind::Closure {
            is_move,
            params,
            body,
        } => format!(
            "{}({}) => {}",
            if *is_move { "move " } else { "" },
            params
                .iter()
                .map(|local| hir.local(*local).name.clone())
                .collect::<Vec<_>>()
                .join(", "),
            render_expr(hir, *body)
        ),
        hir::ExprKind::Call { callee, args } => format!(
            "{}({})",
            render_expr(hir, *callee),
            args.iter()
                .map(|arg| match arg {
                    hir::CallArg::Positional(expr) => render_expr(hir, *expr),
                    hir::CallArg::Named { name, value, .. } => {
                        format!("{name}: {}", render_expr(hir, *value))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::ExprKind::Member { object, field, .. } => {
            format!("{}.{}", render_expr(hir, *object), field)
        }
        hir::ExprKind::Bracket { target, items } => format!(
            "{}[{}]",
            render_expr(hir, *target),
            items
                .iter()
                .map(|expr| render_expr(hir, *expr))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::ExprKind::StructLiteral { path, fields } => format!(
            "{} {{ {} }}",
            render_path(path),
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name, render_expr(hir, field.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::ExprKind::Binary { left, op, right } => format!(
            "{} {} {}",
            render_expr(hir, *left),
            render_binary(*op),
            render_expr(hir, *right)
        ),
        hir::ExprKind::Unary { op, expr } => {
            format!("{}{}", render_unary(*op), render_expr(hir, *expr))
        }
        hir::ExprKind::Question(expr) => format!("{}?", render_expr(hir, *expr)),
    }
}

fn render_pattern(hir: &hir::Module, pattern_id: PatternId) -> String {
    match &hir.pattern(pattern_id).kind {
        hir::PatternKind::Binding(local) => hir.local(*local).name.clone(),
        hir::PatternKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(|pattern| render_pattern(hir, *pattern))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::PatternKind::Path(path) => render_path(path),
        hir::PatternKind::TupleStruct { path, items } => format!(
            "{}({})",
            render_path(path),
            items
                .iter()
                .map(|pattern| render_pattern(hir, *pattern))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hir::PatternKind::Struct {
            path,
            fields,
            has_rest,
        } => {
            let mut rendered = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, render_pattern(hir, field.pattern)))
                .collect::<Vec<_>>();
            if *has_rest {
                rendered.push("..".to_owned());
            }
            format!("{} {{ {} }}", render_path(path), rendered.join(", "))
        }
        hir::PatternKind::Integer(value) => value.clone(),
        hir::PatternKind::String(value) => format!("\"{value}\""),
        hir::PatternKind::Bool(value) => value.to_string(),
        hir::PatternKind::NoneLiteral => "none".to_owned(),
        hir::PatternKind::Wildcard => "_".to_owned(),
    }
}

fn render_binary(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Assign => "=",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::EqEq => "==",
        BinaryOp::BangEq => "!=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
    }
}

fn render_unary(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
        UnaryOp::Neg => "-",
        UnaryOp::Await => "await ",
        UnaryOp::Spawn => "spawn ",
    }
}

fn render_path(path: &ql_ast::Path) -> String {
    path.segments.join(".")
}

fn render_constant(constant: &crate::Constant) -> String {
    match constant {
        crate::Constant::Integer(value) => value.clone(),
        crate::Constant::String { value, is_format } => {
            if *is_format {
                format!("f\"{value}\"")
            } else {
                format!("\"{value}\"")
            }
        }
        crate::Constant::Bool(value) => value.to_string(),
        crate::Constant::None => "none".to_owned(),
        crate::Constant::Void => "Void".to_owned(),
        crate::Constant::Function { name, .. } => name.clone(),
        crate::Constant::Item { name, .. } => name.clone(),
        crate::Constant::Import(path) => render_path(path),
        crate::Constant::UnresolvedName(name) => format!("unresolved({name})"),
    }
}
