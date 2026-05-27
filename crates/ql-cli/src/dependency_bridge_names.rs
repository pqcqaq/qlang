use ql_ast::{FunctionDecl, Param, ReceiverKind, Visibility};

pub(crate) use self::externs::{DependencyExternOwner, record_dependency_extern_declaration};
use self::identifiers::{
    dependency_public_function_export_name, dependency_public_function_local_forwarder_name,
    dependency_public_method_export_name,
};

mod externs;
mod identifiers;

pub(crate) fn supports_dependency_public_function_import_bridge(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public
        && function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && function.generics.is_empty()
        && function.where_clause.is_empty()
        && function
            .params
            .iter()
            .all(|param| matches!(param, Param::Regular { .. }))
}

pub(crate) fn supports_dependency_public_function_export_bridge(function: &FunctionDecl) -> bool {
    supports_dependency_public_function_import_bridge(function) && function.body.is_some()
}

pub(crate) fn render_imported_dependency_public_function_forwarder(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_function_import_bridge(function) {
        return None;
    }
    let params = render_dependency_bridge_param_list(function, contents);
    let args = render_dependency_bridge_arg_list(function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let callable_type = render_dependency_bridge_callable_type(function, contents);
    let export_name =
        dependency_public_function_export_name(module_import_path, function.name.as_str());
    let local_forwarder_name =
        dependency_public_function_local_forwarder_name(module_import_path, function.name.as_str());
    let mut rendered = format!("extern \"c\" fn {export_name}({params}){return_suffix}\n\n");
    rendered.push_str(&format!(
        "fn {local_forwarder_name}({params}){return_suffix} {{\n",
    ));
    if function.return_type.is_some() {
        rendered.push_str(&format!("    return {export_name}({args})\n"));
    } else {
        rendered.push_str(&format!("    {export_name}({args})\n"));
    }
    rendered.push_str("\n}\n\n");
    rendered.push_str(&format!(
        "const {}: {callable_type} = {local_forwarder_name}",
        function.name
    ));
    Some(rendered)
}

pub(crate) fn render_dependency_public_function_export_wrapper(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_function_export_bridge(function) {
        return None;
    }
    let params = render_dependency_bridge_param_list(function, contents);
    let args = render_dependency_bridge_arg_list(function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name =
        dependency_public_function_export_name(module_import_path, function.name.as_str());
    let mut rendered = format!("extern \"c\" pub fn {export_name}({params}){return_suffix} {{\n");
    if function.return_type.is_some() {
        rendered.push_str(&format!("    return {}({args})\n", function.name));
    } else {
        rendered.push_str(&format!("    {}({args})\n", function.name));
    }
    rendered.push('}');
    Some(rendered)
}

pub(crate) fn supports_dependency_public_method_import_bridge(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public
        && function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && function.generics.is_empty()
        && function.where_clause.is_empty()
        && matches!(function.params.first(), Some(Param::Receiver { .. }))
        && function
            .params
            .iter()
            .skip(1)
            .all(|param| matches!(param, Param::Regular { .. }))
}

fn supports_dependency_public_method_export_bridge(function: &FunctionDecl) -> bool {
    supports_dependency_public_method_import_bridge(function) && function.body.is_some()
}

pub(crate) fn render_imported_dependency_public_method_forwarder(
    module_import_path: &[String],
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_method_import_bridge(function) {
        return None;
    }
    let receiver_kind = dependency_bridge_receiver_kind(function)?;
    let ffi_params =
        render_dependency_bridge_method_ffi_param_list(struct_name, function, contents);
    let method_params = render_dependency_bridge_method_param_list(function, contents)?;
    let args = render_dependency_bridge_method_arg_list("self", function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name = dependency_public_method_export_name(
        module_import_path,
        struct_name,
        function.name.as_str(),
    );
    let visibility = render_dependency_bridge_visibility_prefix(&function.visibility);
    let mut rendered = format!("extern \"c\" fn {export_name}({ffi_params}){return_suffix}\n\n");
    rendered.push_str(&format!("impl {struct_name} {{\n"));
    rendered.push_str(&format!(
        "    {visibility}fn {}({method_params}){return_suffix} {{\n",
        function.name
    ));
    let _ = receiver_kind;
    if function.return_type.is_some() {
        rendered.push_str(&format!("        return {export_name}({args})\n"));
    } else {
        rendered.push_str(&format!("        {export_name}({args})\n"));
    }
    rendered.push_str("    }\n}");
    Some(rendered)
}

pub(crate) fn render_dependency_public_method_export_wrapper(
    module_import_path: &[String],
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_method_export_bridge(function) {
        return None;
    }
    let receiver_kind = dependency_bridge_receiver_kind(function)?;
    let ffi_params =
        render_dependency_bridge_method_ffi_param_list(struct_name, function, contents);
    let regular_args = render_dependency_bridge_arg_list(function);
    let method_call_args = if regular_args.is_empty() {
        "()".to_owned()
    } else {
        format!("({regular_args})")
    };
    let receiver_name = match receiver_kind {
        ReceiverKind::Mutable => "bridge_receiver",
        ReceiverKind::ReadOnly | ReceiverKind::Move => "receiver",
    };
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name = dependency_public_method_export_name(
        module_import_path,
        struct_name,
        function.name.as_str(),
    );
    let mut rendered =
        format!("extern \"c\" pub fn {export_name}({ffi_params}){return_suffix} {{\n");
    if matches!(receiver_kind, ReceiverKind::Mutable) {
        rendered.push_str("    var bridge_receiver = receiver\n");
    }
    if function.return_type.is_some() {
        rendered.push_str(&format!(
            "    return {receiver_name}.{}{method_call_args}\n",
            function.name
        ));
    } else {
        rendered.push_str(&format!(
            "    {receiver_name}.{}{method_call_args}\n",
            function.name
        ));
    }
    rendered.push('}');
    Some(rendered)
}

fn render_dependency_bridge_param_list(function: &FunctionDecl, contents: &str) -> String {
    function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, ty, .. } => {
                Some(format!("{name}: {}", span_text(contents, ty.span).trim()))
            }
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dependency_bridge_callable_type(function: &FunctionDecl, contents: &str) -> String {
    let params = function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { ty, .. } => Some(span_text(contents, ty.span).trim().to_owned()),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    let return_ty = function
        .return_type
        .as_ref()
        .map(|ty| span_text(contents, ty.span).trim().to_owned())
        .unwrap_or_else(|| "()".to_owned());
    format!("({params}) -> {return_ty}")
}

fn dependency_bridge_receiver_kind(function: &FunctionDecl) -> Option<ReceiverKind> {
    match function.params.first()? {
        Param::Receiver { kind, .. } => Some(*kind),
        Param::Regular { .. } => None,
    }
}

fn render_dependency_bridge_method_param_list(
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    let receiver =
        render_dependency_bridge_receiver_text(dependency_bridge_receiver_kind(function)?);
    let regular_params = render_dependency_bridge_param_list(function, contents);
    if regular_params.is_empty() {
        Some(receiver.to_owned())
    } else {
        Some(format!("{receiver}, {regular_params}"))
    }
}

fn render_dependency_bridge_method_ffi_param_list(
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> String {
    let regular_params = render_dependency_bridge_param_list(function, contents);
    if regular_params.is_empty() {
        format!("receiver: {struct_name}")
    } else {
        format!("receiver: {struct_name}, {regular_params}")
    }
}

fn render_dependency_bridge_method_arg_list(receiver: &str, function: &FunctionDecl) -> String {
    let regular_args = render_dependency_bridge_arg_list(function);
    if regular_args.is_empty() {
        receiver.to_owned()
    } else {
        format!("{receiver}, {regular_args}")
    }
}

fn render_dependency_bridge_arg_list(function: &FunctionDecl) -> String {
    function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, .. } => Some(name.clone()),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dependency_bridge_return_suffix(function: &FunctionDecl, contents: &str) -> String {
    function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", span_text(contents, ty.span).trim()))
        .unwrap_or_default()
}

fn render_dependency_bridge_receiver_text(kind: ReceiverKind) -> &'static str {
    match kind {
        ReceiverKind::ReadOnly => "self",
        ReceiverKind::Mutable => "var self",
        ReceiverKind::Move => "move self",
    }
}

fn render_dependency_bridge_visibility_prefix(visibility: &Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "",
        Visibility::Public => "pub ",
    }
}

pub(crate) fn span_text(source: &str, span: ql_span::Span) -> String {
    source
        .get(span.start..span.end)
        .unwrap_or_default()
        .to_owned()
}
