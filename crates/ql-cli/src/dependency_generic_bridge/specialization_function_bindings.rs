use ql_ast::Module;

use super::SpecializationModule;
use super::function_bindings::{
    FunctionTypeBindings, collect_imported_function_type_bindings,
    collect_local_function_type_bindings,
};

pub(super) fn collect_root_call_function_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> FunctionTypeBindings {
    let mut bindings = collect_local_function_type_bindings(root_module);
    bindings.extend(collect_imported_function_type_bindings(
        root_module,
        module_import_path,
        dependency_module,
    ));
    bindings
}

pub(super) fn collect_specialization_function_type_bindings(
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> FunctionTypeBindings {
    let mut bindings = collect_local_function_type_bindings(dependency_module);
    collect_specialization_module_local_bindings(&mut bindings, specialization_modules);
    collect_dependency_imported_bindings(&mut bindings, dependency_module, specialization_modules);
    collect_specialization_module_imported_bindings(&mut bindings, specialization_modules);
    bindings
}

fn collect_specialization_module_local_bindings(
    bindings: &mut FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
) {
    for module in specialization_modules {
        bindings.extend(collect_local_function_type_bindings(module.module));
    }
}

fn collect_dependency_imported_bindings(
    bindings: &mut FunctionTypeBindings,
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) {
    for target in specialization_modules {
        bindings.extend(collect_imported_function_type_bindings(
            dependency_module,
            target.module_import_path,
            target.module,
        ));
    }
}

fn collect_specialization_module_imported_bindings(
    bindings: &mut FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
) {
    for caller in specialization_modules {
        for target in specialization_modules {
            bindings.extend(collect_imported_function_type_bindings(
                caller.module,
                target.module_import_path,
                target.module,
            ));
        }
    }
}
