use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{self, FunctionDecl, ItemKind, Module, Param, Visibility};

mod instantiations;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRewrite {
    pub span: ql_span::Span,
    pub replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedPublicFunctionSpecializations {
    pub declarations: String,
    pub call_rewrites: Vec<SourceRewrite>,
}

pub fn supports_public_function_specialization(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public && supports_local_function_specialization(function)
}

pub fn supports_local_function_specialization(function: &FunctionDecl) -> bool {
    function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && !function.generics.is_empty()
        && function.where_clause.is_empty()
        && function
            .params
            .iter()
            .all(|param| matches!(param, Param::Regular { .. }))
}

pub fn render_public_function_specializations(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    dependency_module: &Module,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if !supports_public_function_specialization(function) || function.body.is_none() {
        return None;
    }
    let dependency_function_bindings =
        instantiations::collect_local_function_type_bindings(dependency_module);
    let call_instantiations = instantiations::collect_public_function_call_instantiations(
        root_module,
        module_import_path,
        function,
        &instantiations::collect_imported_function_type_bindings(
            root_module,
            module_import_path,
            dependency_module,
        ),
    );
    render_function_specializations(
        module_import_path,
        function,
        contents,
        dependency_module,
        &dependency_function_bindings,
        call_instantiations,
        rendered_specializations,
    )
}

pub fn render_local_function_specializations(
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if !supports_local_function_specialization(function) || function.body.is_none() {
        return None;
    }
    let module_import_path = root_module
        .package
        .as_ref()
        .map(|package| package.path.segments.as_slice())
        .unwrap_or(&[]);
    let call_instantiations = instantiations::collect_local_function_call_instantiations(
        root_module,
        function,
        &instantiations::collect_local_function_type_bindings(root_module),
    );
    render_function_specializations(
        module_import_path,
        function,
        contents,
        root_module,
        &instantiations::collect_local_function_type_bindings(root_module),
        call_instantiations,
        rendered_specializations,
    )
}

fn render_function_specializations(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &instantiations::FunctionTypeBindings,
    call_instantiations: Vec<instantiations::PublicFunctionCallInstantiation>,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if call_instantiations.is_empty() {
        return None;
    }
    let mut concrete_instantiations = BTreeSet::new();
    for instantiation in &call_instantiations {
        if function
            .generics
            .iter()
            .any(|generic| !instantiation.substitutions.contains_key(&generic.name))
        {
            return None;
        }
        concrete_instantiations.insert(instantiation.substitutions.clone());
    }

    let mut declarations = Vec::new();
    for substitutions in &concrete_instantiations {
        render_public_function_specialized_forwarder(
            module_import_path,
            function,
            contents,
            specialization_module,
            function_bindings,
            substitutions,
            rendered_specializations,
            &mut declarations,
        )?;
    }

    let call_rewrites = call_instantiations
        .into_iter()
        .map(|instantiation| SourceRewrite {
            span: instantiation.callee_span,
            replacement: dependency_public_function_specialized_local_forwarder_name(
                module_import_path,
                &function.name,
                function,
                &instantiation.substitutions,
            ),
        })
        .collect();

    Some(RenderedPublicFunctionSpecializations {
        declarations: declarations.join("\n\n"),
        call_rewrites,
    })
}

fn render_public_function_specialized_forwarder(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &instantiations::FunctionTypeBindings,
    substitutions: &BTreeMap<String, String>,
    rendered_specializations: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
) -> Option<()> {
    let params =
        render_dependency_bridge_param_list_with_substitutions(function, contents, substitutions);
    let return_suffix = render_dependency_bridge_return_suffix_with_substitutions(
        function,
        contents,
        substitutions,
    );
    let specialized_name = dependency_public_function_specialized_local_forwarder_name(
        module_import_path,
        &function.name,
        function,
        substitutions,
    );
    if !rendered_specializations.insert(specialized_name.clone()) {
        return Some(());
    }

    let mut body_call_rewrites = Vec::new();
    for item in &specialization_module.items {
        let ItemKind::Function(callee) = &item.kind else {
            continue;
        };
        if !supports_local_function_specialization(callee) || callee.body.is_none() {
            continue;
        }
        for instantiation in instantiations::collect_specialized_body_call_instantiations(
            function,
            callee,
            substitutions,
            function_bindings,
        ) {
            if callee
                .generics
                .iter()
                .any(|generic| !instantiation.substitutions.contains_key(&generic.name))
            {
                return None;
            }
            render_public_function_specialized_forwarder(
                module_import_path,
                callee,
                contents,
                specialization_module,
                function_bindings,
                &instantiation.substitutions,
                rendered_specializations,
                declarations,
            )?;
            body_call_rewrites.push(SourceRewrite {
                span: instantiation.callee_span,
                replacement: dependency_public_function_specialized_local_forwarder_name(
                    module_import_path,
                    &callee.name,
                    callee,
                    &instantiation.substitutions,
                ),
            });
        }
    }

    let body_span = function.body.as_ref()?.span;
    let body_source = span_text(contents, body_span);
    let leading_trim = body_source.len() - body_source.trim_start().len();
    let body_start = body_span.start + leading_trim;
    let body = apply_specialized_body_rewrites(body_source.trim(), body_start, &body_call_rewrites);
    let body = replace_generic_identifiers(&body, substitutions);

    declarations.push(format!(
        "fn {specialized_name}({params}){return_suffix} {body}"
    ));
    Some(())
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

fn apply_specialized_body_rewrites(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_module(source: &str) -> Module {
        ql_parser::parse_source(source).expect("test source should parse")
    }

    fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
        module
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ItemKind::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .expect("test function should exist")
    }

    #[test]
    fn public_specialization_rewrites_same_dependency_generic_body_calls() {
        let dependency_source = r#"
package dep

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first3[T](values: [T; 3]) -> T {
    return first(values)
}
"#;
        let dependency = parse_module(dependency_source);
        let root = parse_module(
            r#"
use dep.first3 as first3

fn main() -> Int {
    return first3([1, 2, 3])
}
"#,
        );

        let rendered = render_public_function_specializations(
            &["dep".to_owned()],
            function(&dependency, "first3"),
            dependency_source,
            &root,
            &dependency,
            &mut BTreeSet::new(),
        )
        .expect("first3 should render a concrete specialization");

        assert!(
            rendered
                .declarations
                .contains("fn __ql_bridge_local_dep_first__generic_Int_3(values: [Int; 3]) -> Int")
        );
        assert!(
            rendered
                .declarations
                .contains("return __ql_bridge_local_dep_first__generic_Int_3(values)")
        );
        assert!(!rendered.declarations.contains("return first(values)"));
        assert_eq!(rendered.call_rewrites.len(), 1);
        assert_eq!(
            rendered.call_rewrites[0].replacement,
            "__ql_bridge_local_dep_first3__generic_Int"
        );
    }
}
