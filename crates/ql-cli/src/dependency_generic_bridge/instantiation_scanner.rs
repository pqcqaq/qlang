use ql_ast::{self, FunctionDecl, ItemKind};

use super::instantiation_block_scanner::collect_dependency_generic_function_instantiations_from_block;
use super::instantiation_expr_scanner::collect_dependency_generic_function_instantiations_from_expr;
use super::instantiation_scan_context::InstantiationScanContext;
use super::value_bindings::{ValueTypeBindings, collect_function_param_type_bindings};

pub(super) fn collect_dependency_generic_function_instantiations_from_item(
    item: &ql_ast::Item,
    root_bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
) {
    match &item.kind {
        ItemKind::Function(root_function) => {
            collect_callable_body_instantiations(root_function, root_bindings, context);
        }
        ItemKind::Const(global) | ItemKind::Static(global) => {
            collect_dependency_generic_function_instantiations_from_expr(
                &global.value,
                Some(&global.ty),
                None,
                root_bindings,
                context,
            );
        }
        ItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_generic_function_instantiations_from_expr(
                        default,
                        Some(&field.ty),
                        None,
                        root_bindings,
                        context,
                    );
                }
            }
        }
        ItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                collect_callable_body_instantiations(method, root_bindings, context);
            }
        }
        ItemKind::Impl(impl_block) => {
            for method in &impl_block.methods {
                collect_callable_body_instantiations(method, root_bindings, context);
            }
        }
        ItemKind::Extend(extend_block) => {
            for method in &extend_block.methods {
                collect_callable_body_instantiations(method, root_bindings, context);
            }
        }
        ItemKind::Enum(_) | ItemKind::TypeAlias(_) | ItemKind::ExternBlock(_) => {}
    }
}

fn collect_callable_body_instantiations(
    callable: &FunctionDecl,
    root_bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
) {
    let Some(body) = &callable.body else {
        return;
    };
    let mut bindings = root_bindings.clone();
    collect_function_param_type_bindings(callable, &mut bindings);
    collect_dependency_generic_function_instantiations_from_block(
        body,
        &mut bindings,
        context,
        callable.return_type.as_ref(),
        callable.return_type.as_ref(),
    );
}
