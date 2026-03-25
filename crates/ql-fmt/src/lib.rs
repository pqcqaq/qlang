use ql_ast::{
    BinaryOp, Block, CallArg, Expr, FunctionDecl, Item, MatchArm, Module, Param, Pattern,
    PatternField, ReceiverKind, Stmt, StructLiteralField, TypeExpr, VariantFields, Visibility,
};
use ql_parser::{ParseError, parse_source};

pub fn format_source(source: &str) -> Result<String, Vec<ParseError>> {
    let module = parse_source(source)?;
    Ok(format_module(&module))
}

pub fn format_module(module: &Module) -> String {
    let mut out = String::new();

    if let Some(package) = &module.package {
        out.push_str("package ");
        out.push_str(&package.path.segments.join("."));
        out.push('\n');
    }

    if !module.uses.is_empty() {
        if module.package.is_some() {
            out.push('\n');
        }
        for use_decl in &module.uses {
            out.push_str("use ");
            out.push_str(&use_decl.prefix.segments.join("."));
            if let Some(group) = &use_decl.group {
                out.push_str(".{");
                for (idx, item) in group.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&item.name);
                    if let Some(alias) = &item.alias {
                        out.push_str(" as ");
                        out.push_str(alias);
                    }
                }
                out.push('}');
            }
            if let Some(alias) = &use_decl.alias {
                out.push_str(" as ");
                out.push_str(alias);
            }
            out.push('\n');
        }
    }

    if !module.items.is_empty() {
        if module.package.is_some() || !module.uses.is_empty() {
            out.push('\n');
        }

        for (idx, item) in module.items.iter().enumerate() {
            if idx > 0 {
                out.push_str("\n\n");
            }
            format_item(item, 0, &mut out);
        }
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

fn format_item(item: &Item, indent: usize, out: &mut String) {
    match item {
        Item::Function(function) => format_function(function, indent, out),
        Item::Struct(struct_decl) => {
            write_indent(indent, out);
            format_visibility(&struct_decl.visibility, out);
            if struct_decl.is_data {
                out.push_str("data ");
            }
            out.push_str("struct ");
            out.push_str(&struct_decl.name);
            out.push_str(" {\n");
            for field in &struct_decl.fields {
                write_indent(indent + 1, out);
                out.push_str(&field.name);
                out.push_str(": ");
                format_type(&field.ty, out);
                if let Some(default) = &field.default {
                    out.push_str(" = ");
                    format_expr(default, indent + 1, out);
                }
                out.push_str(",\n");
            }
            write_indent(indent, out);
            out.push('}');
        }
        Item::Enum(enum_decl) => {
            write_indent(indent, out);
            format_visibility(&enum_decl.visibility, out);
            out.push_str("enum ");
            out.push_str(&enum_decl.name);
            out.push_str(" {\n");
            for variant in &enum_decl.variants {
                write_indent(indent + 1, out);
                out.push_str(&variant.name);
                match &variant.fields {
                    VariantFields::Unit => {}
                    VariantFields::Tuple(types) => {
                        out.push('(');
                        for (idx, ty) in types.iter().enumerate() {
                            if idx > 0 {
                                out.push_str(", ");
                            }
                            format_type(ty, out);
                        }
                        out.push(')');
                    }
                    VariantFields::Struct(fields) => {
                        out.push_str(" {\n");
                        for field in fields {
                            write_indent(indent + 2, out);
                            out.push_str(&field.name);
                            out.push_str(": ");
                            format_type(&field.ty, out);
                            out.push_str(",\n");
                        }
                        write_indent(indent + 1, out);
                        out.push('}');
                    }
                }
                out.push_str(",\n");
            }
            write_indent(indent, out);
            out.push('}');
        }
        Item::Impl(impl_block) => {
            write_indent(indent, out);
            out.push_str("impl ");
            out.push_str(&impl_block.target.segments.join("."));
            out.push_str(" {\n");
            for (idx, method) in impl_block.methods.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                format_function(method, indent + 1, out);
                out.push('\n');
            }
            if !impl_block.methods.is_empty() {
                out.pop();
            }
            write_indent(indent, out);
            out.push('}');
        }
    }
}

fn format_function(function: &FunctionDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&function.visibility, out);
    if function.is_async {
        out.push_str("async ");
    }
    out.push_str("fn ");
    out.push_str(&function.name);
    out.push('(');
    for (idx, param) in function.params.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        match param {
            Param::Regular { name, ty } => {
                out.push_str(name);
                out.push_str(": ");
                format_type(ty, out);
            }
            Param::Receiver(kind) => match kind {
                ReceiverKind::ReadOnly => out.push_str("self"),
                ReceiverKind::Mutable => out.push_str("var self"),
                ReceiverKind::Move => out.push_str("move self"),
            },
        }
    }
    out.push(')');
    if let Some(ty) = &function.return_type {
        out.push_str(" -> ");
        format_type(ty, out);
    }
    out.push(' ');
    format_block(&function.body, indent, out);
}

fn format_visibility(visibility: &Visibility, out: &mut String) {
    if matches!(visibility, Visibility::Public) {
        out.push_str("pub ");
    }
}

fn format_type(ty: &TypeExpr, out: &mut String) {
    match ty {
        TypeExpr::Named { path, args } => {
            out.push_str(&path.segments.join("."));
            if !args.is_empty() {
                out.push('[');
                for (idx, arg) in args.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    format_type(arg, out);
                }
                out.push(']');
            }
        }
        TypeExpr::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_type(item, out);
            }
            out.push(')');
        }
        TypeExpr::Callable { params, ret } => {
            out.push('(');
            for (idx, param) in params.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_type(param, out);
            }
            out.push_str(") -> ");
            format_type(ret, out);
        }
    }
}

fn format_block(block: &Block, indent: usize, out: &mut String) {
    out.push_str("{\n");
    for stmt in &block.statements {
        format_stmt(stmt, indent + 1, out);
        out.push('\n');
    }
    if let Some(tail) = &block.tail {
        write_indent(indent + 1, out);
        format_expr(tail, indent + 1, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_stmt(stmt: &Stmt, indent: usize, out: &mut String) {
    write_indent(indent, out);
    match stmt {
        Stmt::Let {
            mutable,
            pattern,
            value,
        } => {
            out.push_str(if *mutable { "var " } else { "let " });
            format_pattern(pattern, out);
            out.push_str(" = ");
            format_expr(value, indent, out);
        }
        Stmt::Return(value) => {
            out.push_str("return");
            if let Some(value) = value {
                out.push(' ');
                format_expr(value, indent, out);
            }
        }
        Stmt::Defer(expr) => {
            out.push_str("defer ");
            format_expr(expr, indent, out);
        }
        Stmt::Break => out.push_str("break"),
        Stmt::Continue => out.push_str("continue"),
        Stmt::While { condition, body } => {
            out.push_str("while ");
            format_expr(condition, indent, out);
            out.push(' ');
            format_block(body, indent, out);
        }
        Stmt::Loop { body } => {
            out.push_str("loop ");
            format_block(body, indent, out);
        }
        Stmt::For {
            is_await,
            pattern,
            iterable,
            body,
        } => {
            out.push_str("for ");
            if *is_await {
                out.push_str("await ");
            }
            format_pattern(pattern, out);
            out.push_str(" in ");
            format_expr(iterable, indent, out);
            out.push(' ');
            format_block(body, indent, out);
        }
        Stmt::Expr { expr, .. } => {
            format_expr(expr, indent, out);
            out.push(';');
        }
    }
}

fn format_pattern(pattern: &Pattern, out: &mut String) {
    match pattern {
        Pattern::Name(name) => out.push_str(name),
        Pattern::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_pattern(item, out);
            }
            out.push(')');
        }
        Pattern::Path(path) => out.push_str(&path.segments.join(".")),
        Pattern::TupleStruct { path, items } => {
            out.push_str(&path.segments.join("."));
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_pattern(item, out);
            }
            out.push(')');
        }
        Pattern::Struct {
            path,
            fields,
            has_rest,
        } => format_struct_pattern(path.segments.join(".").as_str(), fields, *has_rest, out),
        Pattern::Integer(value) => out.push_str(value),
        Pattern::String(value) => {
            out.push('"');
            out.push_str(value);
            out.push('"');
        }
        Pattern::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Pattern::NoneLiteral => out.push_str("none"),
        Pattern::Wildcard => out.push('_'),
    }
}

fn format_struct_pattern(path: &str, fields: &[PatternField], has_rest: bool, out: &mut String) {
    out.push_str(path);
    out.push_str(" { ");
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&field.name);
        if let Some(pattern) = &field.pattern {
            out.push_str(": ");
            format_pattern(pattern, out);
        }
    }
    if has_rest {
        if !fields.is_empty() {
            out.push_str(", ");
        }
        out.push_str("..");
    }
    out.push_str(" }");
}

fn format_expr(expr: &Expr, indent: usize, out: &mut String) {
    match expr {
        Expr::Name(name) => out.push_str(name),
        Expr::Integer(value) => out.push_str(value),
        Expr::String { value, is_format } => {
            if *is_format {
                out.push('f');
            }
            out.push('"');
            out.push_str(value);
            out.push('"');
        }
        Expr::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Expr::NoneLiteral => out.push_str("none"),
        Expr::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_expr(item, indent, out);
            }
            out.push(')');
        }
        Expr::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_expr(item, indent, out);
            }
            out.push(']');
        }
        Expr::Block(block) => format_block(block, indent, out),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            out.push_str("if ");
            format_expr(condition, indent, out);
            out.push(' ');
            format_block(then_branch, indent, out);
            if let Some(else_branch) = else_branch {
                out.push_str(" else ");
                format_expr(else_branch, indent, out);
            }
        }
        Expr::Match { value, arms } => format_match_expr(value, arms, indent, out),
        Expr::Closure {
            is_move,
            params,
            body,
        } => {
            if *is_move {
                out.push_str("move ");
            }
            out.push('(');
            for (idx, param) in params.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                out.push_str(param);
            }
            out.push_str(") => ");
            format_expr(body, indent, out);
        }
        Expr::Call { callee, args } => {
            format_expr(callee, indent, out);
            out.push('(');
            for (idx, arg) in args.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                match arg {
                    CallArg::Positional(expr) => format_expr(expr, indent, out),
                    CallArg::Named { name, value } => {
                        out.push_str(name);
                        out.push_str(": ");
                        format_expr(value, indent, out);
                    }
                }
            }
            out.push(')');
        }
        Expr::Member { object, field } => {
            format_expr(object, indent, out);
            out.push('.');
            out.push_str(field);
        }
        Expr::Bracket { target, items } => {
            format_expr(target, indent, out);
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_expr(item, indent, out);
            }
            out.push(']');
        }
        Expr::StructLiteral { path, fields } => {
            out.push_str(&path.segments.join("."));
            out.push_str(" { ");
            for (idx, field) in fields.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_struct_literal_field(field, indent, out);
            }
            out.push_str(" }");
        }
        Expr::Binary { left, op, right } => {
            format_expr(left, indent, out);
            out.push(' ');
            out.push_str(match op {
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
            });
            out.push(' ');
            format_expr(right, indent, out);
        }
        Expr::Unary { op, expr } => {
            out.push_str(match op {
                ql_ast::UnaryOp::Neg => "-",
                ql_ast::UnaryOp::Await => "await ",
                ql_ast::UnaryOp::Spawn => "spawn ",
            });
            format_expr(expr, indent, out);
        }
        Expr::Question(expr) => {
            format_expr(expr, indent, out);
            out.push('?');
        }
    }
}

fn format_match_expr(value: &Expr, arms: &[MatchArm], indent: usize, out: &mut String) {
    out.push_str("match ");
    format_expr(value, indent, out);
    out.push_str(" {\n");
    for arm in arms {
        write_indent(indent + 1, out);
        format_pattern(&arm.pattern, out);
        if let Some(guard) = &arm.guard {
            out.push_str(" if ");
            format_expr(guard, indent + 1, out);
        }
        out.push_str(" => ");
        format_expr(&arm.body, indent + 1, out);
        out.push_str(",\n");
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_struct_literal_field(field: &StructLiteralField, indent: usize, out: &mut String) {
    out.push_str(&field.name);
    if let Some(value) = &field.value {
        out.push_str(": ");
        format_expr(value, indent, out);
    }
}

fn write_indent(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::format_source;

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/parser")
            .join(path)
    }

    #[test]
    fn formatting_is_stable_for_basic_fixture() {
        let source = fs::read_to_string(fixture("pass/basic.ql")).expect("read pass fixture");
        let formatted = format_source(&source).expect("format first pass");
        let reformatted = format_source(&formatted).expect("format second pass");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("data struct User"));
        assert!(formatted.contains("fn div_rem(left: Int, right: Int) -> (Int, Int)"));
    }

    #[test]
    fn formatting_is_stable_for_control_flow_fixture() {
        let source =
            fs::read_to_string(fixture("pass/control_flow.ql")).expect("read control flow fixture");
        let formatted = format_source(&source).expect("format first pass");
        let reformatted = format_source(&formatted).expect("format second pass");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("match command {"));
        assert!(formatted.contains("for await event in stream {"));
        assert!(formatted.contains("tick();"));
    }
}
