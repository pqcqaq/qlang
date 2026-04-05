use ql_analysis::analyze_source;
use ql_runtime::{
    RuntimeCapability, RuntimeHook, RuntimeHookSignature, collect_runtime_hook_signatures,
    runtime_hook_signature,
};

use ql_resolve::BuiltinType;
use ql_span::Span;
use ql_typeck::Ty;

use super::{
    AsyncTaskResultLayout, CodegenInput, CodegenMode, build_async_task_result_layout, emit_module,
};

fn emit(source: &str) -> String {
    emit_with_mode(source, CodegenMode::Program)
}

fn emit_library(source: &str) -> String {
    emit_with_mode(source, CodegenMode::Library)
}

fn emit_with_mode(source: &str, mode: CodegenMode) -> String {
    emit_with_runtime_hooks(source, mode, &[])
}

fn emit_with_runtime_hooks(
    source: &str,
    mode: CodegenMode,
    runtime_hooks: &[RuntimeHookSignature],
) -> String {
    let analysis = analyze_source(source).expect("source should analyze");
    assert!(
        !analysis.has_errors(),
        "test source should not contain semantic diagnostics"
    );

    emit_module(CodegenInput {
        module_name: "test_module",
        mode,
        inline_runtime_support: false,
        hir: analysis.hir(),
        mir: analysis.mir(),
        resolution: analysis.resolution(),
        typeck: analysis.typeck(),
        runtime_hooks,
    })
    .expect("codegen should succeed")
}

fn emit_error(source: &str) -> Vec<String> {
    emit_error_with_runtime_hooks(source, CodegenMode::Program, &[])
}

fn emit_error_with_runtime_hooks(
    source: &str,
    mode: CodegenMode,
    runtime_hooks: &[RuntimeHookSignature],
) -> Vec<String> {
    let analysis = analyze_source(source).expect("source should analyze");
    assert!(
        !analysis.has_errors(),
        "test source should not contain semantic diagnostics"
    );

    emit_module(CodegenInput {
        module_name: "test_module",
        mode,
        inline_runtime_support: false,
        hir: analysis.hir(),
        mir: analysis.mir(),
        resolution: analysis.resolution(),
        typeck: analysis.typeck(),
        runtime_hooks,
    })
    .expect_err("codegen should fail")
    .into_diagnostics()
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect()
}

fn emit_with_runtime_hooks_and_inline_support(
    source: &str,
    mode: CodegenMode,
    runtime_hooks: &[RuntimeHookSignature],
    inline_runtime_support: bool,
) -> String {
    let analysis = analyze_source(source).expect("source should analyze");
    assert!(
        !analysis.has_errors(),
        "test source should not contain semantic diagnostics"
    );

    emit_module(CodegenInput {
        module_name: "test_module",
        mode,
        inline_runtime_support,
        hir: analysis.hir(),
        mir: analysis.mir(),
        resolution: analysis.resolution(),
        typeck: analysis.typeck(),
        runtime_hooks,
    })
    .expect("codegen should succeed")
}

mod async_lowering;
mod async_main_await;
mod async_main_reinit;
mod async_main_smoke;
mod async_main_spawn;
mod async_main_tail;
mod basics;
mod callable_values;
mod cleanup;
mod match_lowering;
