use std::collections::BTreeMap;

use ql_ast::{ItemKind, Module, Path, StructDecl, TypeExpr};

use super::SpecializationModule;
use super::function_bindings::dependency_imported_local_names;
use super::inferred_type_conversion::{
    inferred_type_from_type_expr_with_substitutions, type_expr_from_inferred_type,
};
use super::inferred_types::{InferredType, InferredTypeKind};
use super::substitutions::TypeSubstitutions;

pub(super) type StructTypeBindings = BTreeMap<String, StructDecl>;

pub(super) fn collect_local_struct_type_bindings(module: &Module) -> StructTypeBindings {
    let mut bindings = StructTypeBindings::new();
    for item in &module.items {
        let ItemKind::Struct(struct_decl) = &item.kind else {
            continue;
        };
        bindings.insert(struct_decl.name.clone(), struct_decl.clone());
    }
    bindings
}

pub(super) fn collect_root_call_struct_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> StructTypeBindings {
    let mut bindings = collect_local_struct_type_bindings(root_module);
    bindings.extend(collect_imported_struct_type_bindings(
        root_module,
        module_import_path,
        dependency_module,
    ));
    bindings
}

pub(super) fn collect_root_call_struct_type_bindings_with_specializations(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> StructTypeBindings {
    let mut bindings =
        collect_root_call_struct_type_bindings(root_module, module_import_path, dependency_module);
    for module in specialization_modules {
        bindings.extend(collect_imported_struct_type_bindings(
            root_module,
            module.module_import_path,
            module.module,
        ));
    }
    bindings
}

pub(super) fn collect_specialization_struct_type_bindings(
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> StructTypeBindings {
    let mut bindings = collect_local_struct_type_bindings(dependency_module);
    for module in specialization_modules {
        bindings.extend(collect_local_struct_type_bindings(module.module));
    }
    for target in specialization_modules {
        bindings.extend(collect_imported_struct_type_bindings(
            dependency_module,
            target.module_import_path,
            target.module,
        ));
    }
    for caller in specialization_modules {
        for target in specialization_modules {
            bindings.extend(collect_imported_struct_type_bindings(
                caller.module,
                target.module_import_path,
                target.module,
            ));
        }
    }
    bindings
}

fn collect_imported_struct_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> StructTypeBindings {
    let mut bindings = StructTypeBindings::new();
    for item in &dependency_module.items {
        let ItemKind::Struct(struct_decl) = &item.kind else {
            continue;
        };
        for local_name in
            dependency_imported_local_names(root_module, module_import_path, &struct_decl.name)
        {
            bindings.insert(local_name, struct_decl.clone());
        }
    }
    bindings
}

pub(super) fn struct_literal_field_expected_types(
    literal_path: &Path,
    expected_ty: &TypeExpr,
    struct_bindings: &StructTypeBindings,
) -> Option<BTreeMap<String, TypeExpr>> {
    let expected_ty = InferredType::from_type_expr(expected_ty)?;
    let InferredTypeKind::Named { path, args } = expected_ty.kind else {
        return None;
    };
    if path != literal_path.segments {
        return None;
    }
    let struct_name = path.last()?;
    let struct_decl = struct_bindings.get(struct_name)?;
    if struct_decl.generics.len() != args.len() {
        return None;
    }

    let substitutions = struct_decl
        .generics
        .iter()
        .zip(args.iter())
        .map(|(generic, arg)| (generic.name.clone(), arg.rendered.clone()))
        .collect::<TypeSubstitutions>();
    struct_decl
        .fields
        .iter()
        .map(|field| {
            let field_ty =
                inferred_type_from_type_expr_with_substitutions(&field.ty, &substitutions)?;
            Some((field.name.clone(), type_expr_from_inferred_type(&field_ty)))
        })
        .collect()
}

pub(super) fn struct_field_type(
    struct_ty: &InferredType,
    field_name: &str,
    struct_bindings: &StructTypeBindings,
) -> Option<InferredType> {
    let InferredTypeKind::Named { path, args } = &struct_ty.kind else {
        return None;
    };
    let struct_name = path.last()?;
    let struct_decl = struct_bindings.get(struct_name)?;
    if struct_decl.generics.len() != args.len() {
        return None;
    }
    let substitutions = struct_decl
        .generics
        .iter()
        .zip(args.iter())
        .map(|(generic, arg)| (generic.name.clone(), arg.rendered.clone()))
        .collect::<TypeSubstitutions>();
    struct_decl
        .fields
        .iter()
        .find(|field| field.name == field_name)
        .and_then(|field| {
            inferred_type_from_type_expr_with_substitutions(&field.ty, &substitutions)
        })
}
