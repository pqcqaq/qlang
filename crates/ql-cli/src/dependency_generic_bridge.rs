use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{self, FunctionDecl, Module, Param, Visibility};

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
) -> Option<RenderedPublicFunctionSpecializations> {
    if !supports_public_function_specialization(function) || function.body.is_none() {
        return None;
    }
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
    render_function_specializations(module_import_path, function, contents, call_instantiations)
}

pub fn render_local_function_specializations(
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
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
    render_function_specializations(module_import_path, function, contents, call_instantiations)
}

fn render_function_specializations(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    call_instantiations: Vec<instantiations::PublicFunctionCallInstantiation>,
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
        declarations.push(render_public_function_specialized_forwarder(
            module_import_path,
            function,
            contents,
            substitutions,
        )?);
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
    substitutions: &BTreeMap<String, String>,
) -> Option<String> {
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
    let body = replace_generic_identifiers(
        span_text(contents, function.body.as_ref()?.span).trim(),
        substitutions,
    );

    Some(format!(
        "fn {specialized_name}({params}){return_suffix} {body}"
    ))
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
