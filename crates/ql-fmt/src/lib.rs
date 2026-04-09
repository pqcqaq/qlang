use ql_ast::{
    BinaryOp, Block, CallArg, EnumDecl, Expr, ExprKind, ExtendBlock, ExternBlock, FunctionDecl,
    GenericParam, GlobalDecl, ImplBlock, Item, ItemKind, MatchArm, Module, Param, Path, Pattern,
    PatternField, PatternKind, ReceiverKind, Stmt, StmtKind, StructDecl, StructLiteralField,
    TraitDecl, TypeAliasDecl, TypeExpr, TypeExprKind, VariantFields, Visibility, WherePredicate,
};
use ql_lexer::is_keyword as lexer_is_keyword;
use ql_parser::{ParseError, parse_source};

pub fn format_source(source: &str) -> Result<String, Vec<ParseError>> {
    let module = parse_source(source)?;
    Ok(format_module(&module))
}

pub fn format_module(module: &Module) -> String {
    let mut out = String::new();

    if let Some(package) = &module.package {
        out.push_str("package ");
        format_path(&package.path, &mut out);
        out.push('\n');
    }

    if !module.uses.is_empty() {
        if module.package.is_some() {
            out.push('\n');
        }
        for use_decl in &module.uses {
            out.push_str("use ");
            format_path(&use_decl.prefix, &mut out);
            if let Some(group) = &use_decl.group {
                out.push_str(".{");
                for (idx, item) in group.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    format_ident(&item.name, &mut out);
                    if let Some(alias) = &item.alias {
                        out.push_str(" as ");
                        format_ident(alias, &mut out);
                    }
                }
                out.push('}');
            }
            if let Some(alias) = &use_decl.alias {
                out.push_str(" as ");
                format_ident(alias, &mut out);
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
    match &item.kind {
        ItemKind::Function(function) => format_function(function, indent, true, out),
        ItemKind::Const(global) => format_global("const", global, indent, out),
        ItemKind::Static(global) => format_global("static", global, indent, out),
        ItemKind::Struct(struct_decl) => format_struct(struct_decl, indent, out),
        ItemKind::Enum(enum_decl) => format_enum(enum_decl, indent, out),
        ItemKind::Trait(trait_decl) => format_trait(trait_decl, indent, out),
        ItemKind::Impl(impl_block) => format_impl(impl_block, indent, out),
        ItemKind::Extend(extend_block) => format_extend(extend_block, indent, out),
        ItemKind::TypeAlias(type_alias) => format_type_alias(type_alias, indent, out),
        ItemKind::ExternBlock(extern_block) => format_extern_block(extern_block, indent, out),
    }
}

fn format_global(keyword: &str, global: &GlobalDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&global.visibility, out);
    out.push_str(keyword);
    out.push(' ');
    format_ident(&global.name, out);
    out.push_str(": ");
    format_type(&global.ty, out);
    out.push_str(" = ");
    format_expr(&global.value, indent, out);
}

fn format_struct(struct_decl: &StructDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&struct_decl.visibility, out);
    if struct_decl.is_data {
        out.push_str("data ");
    }
    out.push_str("struct ");
    format_ident(&struct_decl.name, out);
    format_generic_params(&struct_decl.generics, out);
    out.push_str(" {\n");
    for field in &struct_decl.fields {
        write_indent(indent + 1, out);
        format_ident(&field.name, out);
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

fn format_enum(enum_decl: &EnumDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&enum_decl.visibility, out);
    out.push_str("enum ");
    format_ident(&enum_decl.name, out);
    format_generic_params(&enum_decl.generics, out);
    out.push_str(" {\n");
    for variant in &enum_decl.variants {
        write_indent(indent + 1, out);
        format_ident(&variant.name, out);
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
                    format_ident(&field.name, out);
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

fn format_trait(trait_decl: &TraitDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&trait_decl.visibility, out);
    out.push_str("trait ");
    format_ident(&trait_decl.name, out);
    format_generic_params(&trait_decl.generics, out);
    out.push_str(" {\n");
    for method in &trait_decl.methods {
        format_function(method, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_impl(impl_block: &ImplBlock, indent: usize, out: &mut String) {
    write_indent(indent, out);
    out.push_str("impl");
    format_generic_params(&impl_block.generics, out);
    out.push(' ');
    if let Some(trait_ty) = &impl_block.trait_ty {
        format_type(trait_ty, out);
        out.push_str(" for ");
    }
    format_type(&impl_block.target, out);
    format_where_clause(&impl_block.where_clause, indent, out);
    if impl_block.where_clause.is_empty() {
        out.push_str(" {\n");
    } else {
        write_indent(indent, out);
        out.push_str("{\n");
    }
    for (idx, method) in impl_block.methods.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        format_function(method, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_extend(extend_block: &ExtendBlock, indent: usize, out: &mut String) {
    write_indent(indent, out);
    out.push_str("extend ");
    format_type(&extend_block.target, out);
    out.push_str(" {\n");
    for (idx, method) in extend_block.methods.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        format_function(method, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_type_alias(type_alias: &TypeAliasDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&type_alias.visibility, out);
    if type_alias.is_opaque {
        out.push_str("opaque ");
    }
    out.push_str("type ");
    format_ident(&type_alias.name, out);
    format_generic_params(&type_alias.generics, out);
    out.push_str(" = ");
    format_type(&type_alias.ty, out);
}

fn format_extern_block(extern_block: &ExternBlock, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&extern_block.visibility, out);
    out.push_str("extern ");
    out.push('"');
    out.push_str(&extern_block.abi);
    out.push('"');
    out.push_str(" {\n");
    for function in &extern_block.functions {
        format_function(function, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn format_function(function: &FunctionDecl, indent: usize, show_abi: bool, out: &mut String) {
    write_indent(indent, out);
    if show_abi && let Some(abi) = &function.abi {
        out.push_str("extern ");
        out.push('"');
        out.push_str(abi);
        out.push('"');
        out.push(' ');
    }
    format_visibility(&function.visibility, out);
    if function.is_unsafe {
        out.push_str("unsafe ");
    }
    if function.is_async {
        out.push_str("async ");
    }
    out.push_str("fn ");
    format_ident(&function.name, out);
    format_generic_params(&function.generics, out);
    out.push('(');
    for (idx, param) in function.params.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        match param {
            Param::Regular { name, ty, .. } => {
                format_ident(name, out);
                out.push_str(": ");
                format_type(ty, out);
            }
            Param::Receiver { kind, .. } => match kind {
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
    format_where_clause(&function.where_clause, indent, out);
    if let Some(body) = &function.body {
        if function.where_clause.is_empty() {
            out.push(' ');
        } else {
            write_indent(indent, out);
        }
        format_block(body, indent, out);
    }
}

fn format_generic_params(params: &[GenericParam], out: &mut String) {
    if params.is_empty() {
        return;
    }

    out.push('[');
    for (idx, param) in params.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        format_ident(&param.name, out);
        if !param.bounds.is_empty() {
            out.push_str(": ");
            for (bound_idx, bound) in param.bounds.iter().enumerate() {
                if bound_idx > 0 {
                    out.push_str(" + ");
                }
                format_path(bound, out);
            }
        }
    }
    out.push(']');
}

fn format_where_clause(predicates: &[WherePredicate], indent: usize, out: &mut String) {
    if predicates.is_empty() {
        return;
    }

    out.push('\n');
    write_indent(indent, out);
    out.push_str("where\n");
    for (idx, predicate) in predicates.iter().enumerate() {
        write_indent(indent + 1, out);
        format_type(&predicate.target, out);
        out.push_str(": ");
        for (bound_idx, bound) in predicate.bounds.iter().enumerate() {
            if bound_idx > 0 {
                out.push_str(" + ");
            }
            format_path(bound, out);
        }
        if idx + 1 != predicates.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
}

fn format_visibility(visibility: &Visibility, out: &mut String) {
    if matches!(visibility, Visibility::Public) {
        out.push_str("pub ");
    }
}

fn format_type(ty: &TypeExpr, out: &mut String) {
    match &ty.kind {
        TypeExprKind::Pointer { is_const, inner } => {
            out.push('*');
            if *is_const {
                out.push_str("const ");
            }
            format_type(inner, out);
        }
        TypeExprKind::Array { element, len } => {
            out.push('[');
            format_type(element, out);
            out.push_str("; ");
            out.push_str(len);
            out.push(']');
        }
        TypeExprKind::Named { path, args } => {
            format_path(path, out);
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
        TypeExprKind::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_type(item, out);
            }
            if items.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        TypeExprKind::Callable { params, ret } => {
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
    match &stmt.kind {
        StmtKind::Let {
            mutable,
            pattern,
            ty,
            value,
        } => {
            out.push_str(if *mutable { "var " } else { "let " });
            format_pattern(pattern, out);
            if let Some(ty) = ty {
                out.push_str(": ");
                format_type(ty, out);
            }
            out.push_str(" = ");
            format_expr(value, indent, out);
        }
        StmtKind::Return(value) => {
            out.push_str("return");
            if let Some(value) = value {
                out.push(' ');
                format_expr(value, indent, out);
            }
        }
        StmtKind::Defer(expr) => {
            out.push_str("defer ");
            format_expr(expr, indent, out);
        }
        StmtKind::Break => out.push_str("break"),
        StmtKind::Continue => out.push_str("continue"),
        StmtKind::While { condition, body } => {
            out.push_str("while ");
            format_expr(condition, indent, out);
            out.push(' ');
            format_block(body, indent, out);
        }
        StmtKind::Loop { body } => {
            out.push_str("loop ");
            format_block(body, indent, out);
        }
        StmtKind::For {
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
        StmtKind::Expr { expr, terminated } => {
            format_expr(expr, indent, out);
            if *terminated {
                out.push(';');
            }
        }
    }
}

fn format_pattern(pattern: &Pattern, out: &mut String) {
    match &pattern.kind {
        PatternKind::Name(name) => format_ident(name, out),
        PatternKind::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_pattern(item, out);
            }
            out.push(')');
        }
        PatternKind::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_pattern(item, out);
            }
            out.push(']');
        }
        PatternKind::Path(path) => format_path(path, out),
        PatternKind::TupleStruct { path, items } => {
            format_path(path, out);
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_pattern(item, out);
            }
            out.push(')');
        }
        PatternKind::Struct {
            path,
            fields,
            has_rest,
        } => format_struct_pattern(path, fields, *has_rest, out),
        PatternKind::Integer(value) => out.push_str(value),
        PatternKind::String(value) => {
            out.push('"');
            out.push_str(value);
            out.push('"');
        }
        PatternKind::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        PatternKind::NoneLiteral => out.push_str("none"),
        PatternKind::Wildcard => out.push('_'),
    }
}

fn format_struct_pattern(path: &Path, fields: &[PatternField], has_rest: bool, out: &mut String) {
    format_path(path, out);
    out.push_str(" { ");
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        format_ident(&field.name, out);
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
    match &expr.kind {
        ExprKind::Name(name) => {
            if name == "self" {
                out.push_str("self");
            } else {
                format_ident(name, out);
            }
        }
        ExprKind::Integer(value) => out.push_str(value),
        ExprKind::String { value, is_format } => {
            if *is_format {
                out.push('f');
            }
            out.push('"');
            out.push_str(value);
            out.push('"');
        }
        ExprKind::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        ExprKind::NoneLiteral => out.push_str("none"),
        ExprKind::Tuple(items) => {
            out.push('(');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_expr(item, indent, out);
            }
            if items.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        ExprKind::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_expr(item, indent, out);
            }
            out.push(']');
        }
        ExprKind::Block(block) => format_block(block, indent, out),
        ExprKind::Unsafe(block) => {
            out.push_str("unsafe ");
            format_block(block, indent, out);
        }
        ExprKind::If {
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
        ExprKind::Match { value, arms } => format_match_expr(value, arms, indent, out),
        ExprKind::Closure {
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
                out.push_str(&param.name);
            }
            out.push_str(") => ");
            format_expr(body, indent, out);
        }
        ExprKind::Call { callee, args } => {
            format_expr(callee, indent, out);
            out.push('(');
            for (idx, arg) in args.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                match arg {
                    CallArg::Positional(expr) => format_expr(expr, indent, out),
                    CallArg::Named { name, value, .. } => {
                        format_ident(name, out);
                        out.push_str(": ");
                        format_expr(value, indent, out);
                    }
                }
            }
            out.push(')');
        }
        ExprKind::Member { object, field, .. } => {
            format_expr(object, indent, out);
            out.push('.');
            format_ident(field, out);
        }
        ExprKind::Bracket { target, items } => {
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
        ExprKind::StructLiteral { path, fields } => {
            format_path(path, out);
            out.push_str(" { ");
            for (idx, field) in fields.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_struct_literal_field(field, indent, out);
            }
            out.push_str(" }");
        }
        ExprKind::Binary { left, op, right } => {
            format_expr(left, indent, out);
            out.push(' ');
            out.push_str(match op {
                BinaryOp::Assign => "=",
                BinaryOp::OrOr => "||",
                BinaryOp::AndAnd => "&&",
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
        ExprKind::Unary { op, expr } => {
            out.push_str(match op {
                ql_ast::UnaryOp::Not => "!",
                ql_ast::UnaryOp::Neg => "-",
                ql_ast::UnaryOp::Await => "await ",
                ql_ast::UnaryOp::Spawn => "spawn ",
            });
            format_expr(expr, indent, out);
        }
        ExprKind::Question(expr) => {
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
    format_ident(&field.name, out);
    if let Some(value) = &field.value {
        out.push_str(": ");
        format_expr(value, indent, out);
    }
}

fn format_path(path: &Path, out: &mut String) {
    for (idx, segment) in path.segments.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        format_ident(segment, out);
    }
}

fn format_ident(name: &str, out: &mut String) {
    if needs_identifier_escape(name) {
        out.push('`');
        out.push_str(name);
        out.push('`');
    } else {
        out.push_str(name);
    }
}

fn needs_identifier_escape(name: &str) -> bool {
    lexer_is_keyword(name)
        || name.is_empty()
        || !name.chars().next().is_some_and(is_ident_start)
        || !name.chars().all(is_ident_continue)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
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

    fn assert_format_stable(path: &str) {
        let source = fs::read_to_string(fixture(path)).expect("read pass fixture");
        let formatted = format_source(&source).expect("format first pass");
        let reformatted = format_source(&formatted).expect("format second pass");

        assert_eq!(formatted, reformatted);
    }

    #[test]
    fn formatting_is_stable_for_basic_fixture() {
        assert_format_stable("pass/basic.ql");
    }

    #[test]
    fn formatting_is_stable_for_control_flow_fixture() {
        assert_format_stable("pass/control_flow.ql");
    }

    #[test]
    fn formatting_is_stable_for_phase1_declarations_fixture() {
        let source =
            fs::read_to_string(fixture("pass/phase1_declarations.ql")).expect("read pass fixture");
        let formatted = format_source(&source).expect("format first pass");
        let reformatted = format_source(&formatted).expect("format second pass");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("pub trait Writer[T: io.Flush]"));
        assert!(formatted.contains("self.value = value"));
        assert!(formatted.contains("pub extern \"c\" {"));
        assert!(formatted.contains("fn strlen(ptr: *const U8) -> USize"));
        assert!(formatted.contains("extern \"c\" pub unsafe fn q_add"));
        assert!(formatted.contains("pub struct Tagged {\n    `type`: String,\n}"));
        assert!(formatted.contains("let _value = `type`"));
        assert!(formatted.contains("opaque type UserId = U64"));
    }

    #[test]
    fn formatter_round_trips_extern_function_definitions() {
        let formatted = format_source(
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        )
        .expect("format extern definition");
        let reformatted = format_source(&formatted).expect("format extern definition again");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("extern \"c\" pub fn q_add"));
        assert!(formatted.contains("{\n    return left + right\n}"));
    }

    #[test]
    fn formatter_escapes_keyword_identifiers() {
        let formatted = format_source(
            r#"
fn keyword_passthrough(`type`: String) -> String {
    let _value = `type`
    return _value
}
"#,
        )
        .expect("format keyword identifiers");

        assert!(formatted.contains("fn keyword_passthrough(`type`: String) -> String"));
        assert!(formatted.contains("let _value = `type`"));
    }

    #[test]
    fn formatter_preserves_single_element_tuples() {
        let formatted = format_source(
            r#"
fn tupled(value: (Int,)) -> (Int,) {
    let single = (value,)
    return single
}
"#,
        )
        .expect("format tuple forms");
        let reformatted = format_source(&formatted).expect("format tuple forms again");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("fn tupled(value: (Int,)) -> (Int,)"));
        assert!(formatted.contains("let single = (value,)"));
    }

    #[test]
    fn formatter_preserves_array_type_syntax() {
        let formatted = format_source(
            r#"
fn takes(values:[Int;0x3])->[Int;0x3]{
    return values
}
"#,
        )
        .expect("format array types");
        let reformatted = format_source(&formatted).expect("format array types again");

        assert_eq!(formatted, reformatted);
        assert!(formatted.contains("fn takes(values: [Int; 0x3]) -> [Int; 0x3]"));
    }
}
