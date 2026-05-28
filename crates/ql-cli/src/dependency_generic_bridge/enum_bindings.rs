use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{EnumDecl, ItemKind, Module, VariantFields};

use super::SpecializationModule;
use super::function_bindings::dependency_imported_local_names;
use super::inferred_type_conversion::{
    inferred_type_from_rendered_substitution, inferred_type_from_type_expr_with_substitutions,
};
use super::inferred_types::InferredType;
use super::substitutions::{TypeSubstitutions, collect_generic_type_substitutions};

pub(super) type EnumTypeBindings = BTreeMap<String, EnumDecl>;

pub(super) fn collect_local_enum_type_bindings(module: &Module) -> EnumTypeBindings {
    let mut bindings = EnumTypeBindings::new();
    for item in &module.items {
        let ItemKind::Enum(enum_decl) = &item.kind else {
            continue;
        };
        bindings.insert(enum_decl.name.clone(), enum_decl.clone());
    }
    bindings
}

pub(super) fn collect_root_call_enum_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> EnumTypeBindings {
    let mut bindings = collect_local_enum_type_bindings(root_module);
    bindings.extend(collect_imported_enum_type_bindings(
        root_module,
        module_import_path,
        dependency_module,
    ));
    bindings
}

pub(super) fn collect_specialization_enum_type_bindings(
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> EnumTypeBindings {
    let mut bindings = collect_local_enum_type_bindings(dependency_module);
    for module in specialization_modules {
        bindings.extend(collect_local_enum_type_bindings(module.module));
    }
    for target in specialization_modules {
        bindings.extend(collect_imported_enum_type_bindings(
            dependency_module,
            target.module_import_path,
            target.module,
        ));
    }
    for caller in specialization_modules {
        for target in specialization_modules {
            bindings.extend(collect_imported_enum_type_bindings(
                caller.module,
                target.module_import_path,
                target.module,
            ));
        }
    }
    bindings
}

fn collect_imported_enum_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> EnumTypeBindings {
    let mut bindings = EnumTypeBindings::new();
    for item in &dependency_module.items {
        let ItemKind::Enum(enum_decl) = &item.kind else {
            continue;
        };
        for local_name in
            dependency_imported_local_names(root_module, module_import_path, &enum_decl.name)
        {
            bindings.insert(local_name, enum_decl.clone());
        }
    }
    bindings
}

pub(super) fn infer_tuple_variant_enum_type(
    enum_name: &str,
    variant_name: &str,
    arg_types: &[InferredType],
    enum_bindings: &EnumTypeBindings,
) -> Option<InferredType> {
    let enum_decl = enum_bindings.get(enum_name)?;
    let variant = enum_decl
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?;
    let VariantFields::Tuple(fields) = &variant.fields else {
        return None;
    };
    if fields.len() != arg_types.len() {
        return None;
    }

    let generic_names = enum_decl
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = TypeSubstitutions::new();
    for (field, arg_ty) in fields.iter().zip(arg_types) {
        if !collect_generic_type_substitutions(field, arg_ty, &generic_names, &mut substitutions) {
            return None;
        }
    }

    let args = enum_decl
        .generics
        .iter()
        .map(|generic| substitutions.get(&generic.name))
        .map(|rendered| rendered.map(|rendered| inferred_type_from_rendered_substitution(rendered)))
        .collect::<Option<Vec<_>>>()?;
    Some(InferredType::named(vec![enum_name.to_owned()], args))
}

pub(super) fn tuple_variant_field_types(
    enum_name: &str,
    variant_name: &str,
    enum_args: &[InferredType],
    enum_bindings: &EnumTypeBindings,
) -> Option<Vec<InferredType>> {
    let enum_decl = enum_bindings.get(enum_name)?;
    if enum_decl.generics.len() != enum_args.len() {
        return None;
    }
    let variant = enum_decl
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?;
    let VariantFields::Tuple(fields) = &variant.fields else {
        return None;
    };

    let substitutions = enum_decl
        .generics
        .iter()
        .zip(enum_args)
        .map(|(generic, arg)| (generic.name.clone(), arg.rendered.clone()))
        .collect::<TypeSubstitutions>();
    fields
        .iter()
        .map(|field| inferred_type_from_type_expr_with_substitutions(field, &substitutions))
        .collect()
}
