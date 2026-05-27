use std::collections::BTreeMap;

use ql_ast::{FunctionDecl, Param};

use super::SourceRewrite;

pub(super) fn render_dependency_bridge_generic_params_with_substitutions(
    function: &FunctionDecl,
    substitutions: &BTreeMap<String, String>,
) -> String {
    let preserved = function
        .generics
        .iter()
        .filter_map(|generic| {
            substitutions
                .get(&generic.name)
                .filter(|replacement| *replacement == &generic.name)
                .map(|_| generic.name.as_str())
        })
        .collect::<Vec<_>>();
    if preserved.is_empty() {
        return String::new();
    }
    format!("[{}]", preserved.join(", "))
}

pub(super) fn render_dependency_bridge_param_list_with_substitutions(
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

pub(super) fn render_dependency_bridge_return_suffix_with_substitutions(
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

pub(super) fn apply_specialized_body_rewrites(
    body: &str,
    body_start: usize,
    source_rewrites: &[SourceRewrite],
) -> String {
    let mut rewrites = source_rewrites.to_vec();
    rewrites.sort_by(|left, right| {
        right
            .span
            .start
            .cmp(&left.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
    });

    let mut rewritten = body.to_owned();
    let body_end = body_start + body.len();
    let mut next_start = body_end;
    for rewrite in rewrites {
        if rewrite.span.start < body_start
            || rewrite.span.start > rewrite.span.end
            || rewrite.span.end > body_end
            || rewrite.span.end > next_start
        {
            continue;
        }
        let start = rewrite.span.start - body_start;
        let end = rewrite.span.end - body_start;
        if !rewritten.is_char_boundary(start) || !rewritten.is_char_boundary(end) {
            continue;
        }
        rewritten.replace_range(start..end, &rewrite.replacement);
        next_start = rewrite.span.start;
    }
    rewritten
}

pub(super) fn replace_generic_identifiers(
    text: &str,
    substitutions: &BTreeMap<String, String>,
) -> String {
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

pub(super) fn dependency_public_function_specialized_local_forwarder_name(
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

pub(super) fn span_text(source: &str, span: ql_span::Span) -> String {
    source
        .get(span.start..span.end)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
#[path = "rendering_tests.rs"]
mod tests;
