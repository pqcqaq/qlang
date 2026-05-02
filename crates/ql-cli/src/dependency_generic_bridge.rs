use std::collections::BTreeMap;

use ql_ast::{self, FunctionDecl, Module, Param, Visibility};

mod instantiations;

pub fn supports_public_function_specialization(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public
        && function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && !function.generics.is_empty()
        && function.where_clause.is_empty()
        && function
            .params
            .iter()
            .all(|param| matches!(param, Param::Regular { .. }))
}

pub fn render_public_function_specialization(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
) -> Option<String> {
    if !supports_public_function_specialization(function) || function.body.is_none() {
        return None;
    }
    let instantiations = instantiations::collect_public_function_instantiations(
        root_module,
        module_import_path,
        function,
    );
    if instantiations.len() != 1 {
        return None;
    }
    let substitutions = instantiations.into_iter().next()?;
    if function
        .generics
        .iter()
        .any(|generic| !substitutions.contains_key(&generic.name))
    {
        return None;
    }

    let params =
        render_dependency_bridge_param_list_with_substitutions(function, contents, &substitutions);
    let return_suffix = render_dependency_bridge_return_suffix_with_substitutions(
        function,
        contents,
        &substitutions,
    );
    let callable_type = render_dependency_bridge_callable_type_with_substitutions(
        function,
        contents,
        &substitutions,
    );
    let specialized_name = dependency_public_function_specialized_local_forwarder_name(
        module_import_path,
        &function.name,
        function,
        &substitutions,
    );
    let body = replace_generic_identifiers(
        span_text(contents, function.body.as_ref()?.span).trim(),
        &substitutions,
    );

    let mut rendered = format!("fn {specialized_name}({params}){return_suffix} {body}\n\n");
    rendered.push_str(&format!(
        "const {}: {callable_type} = {specialized_name}",
        function.name
    ));
    Some(rendered)
}

fn render_dependency_bridge_param_list_with_substitutions(
    function: &FunctionDecl,
    contents: &str,
    substitutions: &BTreeMap<String, String>,
) -> String {
    function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, ty, .. } => Some(format!(
                "{name}: {}",
                render_dependency_bridge_type_with_substitutions(ty, contents, substitutions)
            )),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dependency_bridge_callable_type_with_substitutions(
    function: &FunctionDecl,
    contents: &str,
    substitutions: &BTreeMap<String, String>,
) -> String {
    let params =
        function
            .params
            .iter()
            .filter_map(|param| match param {
                Param::Regular { ty, .. } => Some(
                    render_dependency_bridge_type_with_substitutions(ty, contents, substitutions),
                ),
                Param::Receiver { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(", ");
    let return_ty = function
        .return_type
        .as_ref()
        .map(|ty| render_dependency_bridge_type_with_substitutions(ty, contents, substitutions))
        .unwrap_or_else(|| "()".to_owned());
    format!("({params}) -> {return_ty}")
}

fn render_dependency_bridge_return_suffix_with_substitutions(
    function: &FunctionDecl,
    contents: &str,
    substitutions: &BTreeMap<String, String>,
) -> String {
    function
        .return_type
        .as_ref()
        .map(|ty| {
            format!(
                " -> {}",
                render_dependency_bridge_type_with_substitutions(ty, contents, substitutions)
            )
        })
        .unwrap_or_default()
}

fn render_dependency_bridge_type_with_substitutions(
    ty: &ql_ast::TypeExpr,
    contents: &str,
    substitutions: &BTreeMap<String, String>,
) -> String {
    replace_generic_identifiers(span_text(contents, ty.span).trim(), substitutions)
}

fn replace_generic_identifiers(text: &str, substitutions: &BTreeMap<String, String>) -> String {
    let mut rendered = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch == '_' || ch.is_ascii_alphabetic() {
            let mut end = start + ch.len_utf8();
            while let Some((next_index, next_ch)) = chars.peek().copied() {
                if next_ch == '_' || next_ch.is_ascii_alphanumeric() {
                    chars.next();
                    end = next_index + next_ch.len_utf8();
                } else {
                    break;
                }
            }
            let ident = &text[start..end];
            if let Some(replacement) = substitutions.get(ident) {
                rendered.push_str(replacement);
            } else {
                rendered.push_str(ident);
            }
            continue;
        }
        rendered.push(ch);
    }
    rendered
}

fn dependency_public_function_specialized_local_forwarder_name(
    module_import_path: &[String],
    symbol_name: &str,
    function: &FunctionDecl,
    substitutions: &BTreeMap<String, String>,
) -> String {
    let mut rendered = String::from("__ql_bridge_local_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered.push_str("__generic");
    for generic in &function.generics {
        rendered.push('_');
        let ty = substitutions
            .get(&generic.name)
            .map(String::as_str)
            .unwrap_or(generic.name.as_str());
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(ty));
    }
    rendered
}

fn sanitize_dependency_bridge_identifier_fragment(fragment: &str) -> String {
    let mut rendered = String::new();
    for ch in fragment.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            rendered.push(ch);
        } else {
            rendered.push('_');
        }
    }
    if rendered.is_empty() {
        rendered.push('_');
    }
    rendered
}

fn span_text(source: &str, span: ql_span::Span) -> String {
    source
        .get(span.start..span.end)
        .unwrap_or_default()
        .to_owned()
}
