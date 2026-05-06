use super::*;

#[test]
fn emits_llvm_ir_for_direct_calls_and_arithmetic() {
    let rendered = emit(
        r#"
fn add_one(value: Int) -> Int {
return value + 1
}

fn main() -> Int {
let value = add_one(41)
return value
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_add_one(i64 %arg0)"));
    assert!(rendered.contains("define i64 @ql_1_main()"));
    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call i64 @ql_0_add_one(i64 41)"));
    assert!(rendered.contains("call i64 @ql_1_main()"));
    assert!(rendered.contains("add i64"));
}

#[test]
fn emits_llvm_ir_for_named_call_arguments_in_signature_order() {
    let rendered = emit(
        r#"
fn collect(values: [Int; 0], left: Int, right: Int) -> Int {
return left + right + 5
}

fn main() -> Int {
return collect(right: 20, values: [], left: 22)
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_collect([0 x i64] %arg0, i64 %arg1, i64 %arg2)"));
    assert!(rendered.contains("store [0 x i64] zeroinitializer"));
    assert!(rendered.contains("call i64 @ql_0_collect([0 x i64] %t0, i64 22, i64 20)"));
    assert!(!rendered.contains("does not support named call arguments yet"));
}

#[test]
fn emits_llvm_ir_for_same_file_import_alias_named_calls() {
    let rendered = emit(
        r#"
use collect as run

fn collect(values: [Int; 0], left: Int, right: Int) -> Int {
return left + right + 7
}

fn main() -> Int {
return run(right: 20, values: [], left: 22)
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_collect([0 x i64] %arg0, i64 %arg1, i64 %arg2)"));
    assert!(rendered.contains("call i64 @ql_0_collect([0 x i64] %t0, i64 22, i64 20)"));
    assert!(!rendered.contains("only supports direct resolved function calls"));
}

#[test]
fn emits_branches_for_bool_conditions() {
    let rendered = emit(
        r#"
fn main() -> Int {
if true {
    return 1
}
return 0
}
"#,
    );

    assert!(rendered.contains("br i1 true"));
    assert!(rendered.contains("ret i64"));
    assert!(!rendered.contains("store void void"));
}

#[test]
fn emits_zero_exit_code_wrapper_for_void_main() {
    let rendered = emit(
        r#"
fn main() -> Void {
return
}
"#,
    );

    assert!(rendered.contains("define void @ql_0_main()"));
    assert!(rendered.contains("call void @ql_0_main()"));
    assert!(rendered.contains("ret i32 0"));
}

#[test]
fn library_mode_exports_free_functions_without_host_main_wrapper() {
    let rendered = emit_library(
        r#"
fn add_one(value: Int) -> Int {
return value + 1
}

fn add_two(value: Int) -> Int {
return add_one(add_one(value))
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_add_one(i64 %arg0)"));
    assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
    assert!(!rendered.contains("define i32 @main()"));
}

#[test]
fn emits_extern_c_declarations_for_direct_calls() {
    let rendered = emit(
        r#"
extern "c" {
fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
return q_add(1, 2)
}
"#,
    );

    assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
    assert!(rendered.contains("define i64 @ql_1_main()"));
    assert!(rendered.contains("call i64 @q_add(i64 1, i64 2)"));
}

#[test]
fn emits_string_literal_globals_and_string_value_transport() {
    let rendered = emit_library(
        r#"
const GREETING: String = "hello"

fn direct() -> String {
let local = "hello"
return local
}

fn from_const() -> String {
return GREETING
}
"#,
    );

    assert_eq!(
        rendered
            .matches(r#"private unnamed_addr constant [6 x i8] c"\68\65\6C\6C\6F\00""#)
            .count(),
        1
    );
    assert!(rendered.contains("@ql_str_0 = private unnamed_addr constant [6 x i8]"));
    assert!(rendered.contains("define { ptr, i64 } @ql_1_direct()"));
    assert!(rendered.contains("define { ptr, i64 } @ql_2_from_const()"));
    assert!(rendered.contains("getelementptr inbounds [6 x i8], ptr @ql_str_0, i32 0, i32 0"));
    assert!(rendered.contains("insertvalue { ptr, i64 } undef, ptr"));
    assert!(rendered.contains("insertvalue { ptr, i64 } %"));
    assert!(rendered.contains("i64 5, 1"));
}

#[test]
fn emits_string_equality_lowering_via_length_check_and_memcmp() {
    let rendered = emit_library(
        r#"
fn same(left: String, right: String) -> Bool {
return left == right
}

fn different() -> Bool {
return "alpha" != "beta"
}
"#,
    );

    assert!(rendered.contains("declare i32 @memcmp(ptr, ptr, i64)"));
    assert!(rendered.matches("extractvalue { ptr, i64 }").count() >= 4);
    assert!(rendered.contains("icmp eq i64"));
    assert!(rendered.contains("call i32 @memcmp(ptr"));
    assert!(rendered.contains("icmp eq i32"));
    assert!(rendered.contains("icmp ne i32"));
    assert!(rendered.contains("phi i1 [ false"));
    assert!(rendered.contains("phi i1 [ true"));
}

#[test]
fn emits_string_ordered_comparisons_via_memcmp_and_length_tiebreak() {
    let rendered = emit_library(
        r#"
fn less(left: String, right: String) -> Bool {
return left < right
}

fn at_least(left: String, right: String) -> Bool {
return left >= right
}
"#,
    );

    assert!(rendered.contains("declare i32 @memcmp(ptr, ptr, i64)"));
    assert!(rendered.contains("select i1"));
    assert!(rendered.contains("icmp eq i32"));
    assert!(rendered.contains("icmp slt i32"));
    assert!(rendered.contains("icmp sgt i32"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("icmp uge i64"));
    assert!(rendered.contains("phi i1 [ %"));
}

#[test]
fn emits_runtime_hook_declarations_from_shared_abi_contract() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncFunctionBodies,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
fn main() -> Int {
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    let async_frame_alloc = "define ptr @qlrt_async_frame_alloc(i64 %size, i64 %align)";
    let async_task_create = "define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)";
    let executor_spawn = "define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)";
    let task_await = "define ptr @qlrt_task_await(ptr %handle)";
    let task_result_release = "define void @qlrt_task_result_release(ptr %result)";
    let entry_definition = "define i64 @ql_0_main()";

    assert!(rendered.contains(async_frame_alloc));
    assert!(rendered.contains(async_task_create));
    assert!(rendered.contains(executor_spawn));
    assert!(rendered.contains(task_await));
    assert!(rendered.contains(task_result_release));
    assert!(
        rendered
            .find(async_frame_alloc)
            .expect("runtime definition should exist")
            < rendered
                .find(entry_definition)
                .expect("entry function should exist")
    );
    assert!(
        rendered
            .find(async_task_create)
            .expect("runtime definition should exist")
            < rendered
                .find(entry_definition)
                .expect("entry function should exist")
    );
    assert!(
        rendered
            .find(executor_spawn)
            .expect("runtime definition should exist")
            < rendered
                .find(entry_definition)
                .expect("entry function should exist")
    );
    assert!(
        rendered
            .find(task_await)
            .expect("runtime definition should exist")
            < rendered
                .find(entry_definition)
                .expect("entry function should exist")
    );
    assert!(
        rendered
            .find(task_result_release)
            .expect("runtime definition should exist")
            < rendered
                .find(entry_definition)
                .expect("entry function should exist")
    );
}

#[test]
fn emits_async_task_create_wrapper_for_parameterless_async_body() {
    let runtime_hooks = collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_async_frame_alloc(i64, i64)"));
    assert!(rendered.contains("declare ptr @qlrt_async_task_create(ptr, ptr)"));
    assert!(rendered.contains("define i64 @ql_0_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("define ptr @ql_0_worker__async_entry(ptr %frame)"));
    assert!(rendered.contains("call i64 @ql_0_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @malloc(i64 8)"));
    assert!(rendered.contains("define ptr @ql_0_worker()"));
    assert!(
        rendered
            .contains("call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_entry, ptr null)")
    );
}

#[test]
fn emits_async_task_create_wrapper_with_heap_frame_for_parameterized_async_body() {
    let runtime_hooks = collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(flag: Bool, value: Int) -> Int {
if flag {
    return value
}
return 0
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_async_frame_alloc(i64, i64)"));
    assert!(rendered.contains("define i64 @ql_0_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("define ptr @ql_0_worker__async_entry(ptr %frame)"));
    assert!(rendered.contains("call i64 @ql_0_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @malloc(i64 8)"));
    assert!(rendered.contains("define ptr @ql_0_worker(i1 %arg0, i64 %arg1)"));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 16, i64 8)"));
    assert!(
        rendered.contains("getelementptr inbounds { i1, i64 }, ptr %async_frame, i32 0, i32 0")
    );
    assert!(
        rendered.contains("getelementptr inbounds { i1, i64 }, ptr %async_frame, i32 0, i32 1")
    );
    assert!(rendered.contains("store i1 %arg0, ptr %async_frame_field0"));
    assert!(rendered.contains("store i64 %arg1, ptr %async_frame_field1"));
    assert!(rendered.contains(
        "call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_entry, ptr %async_frame)"
    ));
    assert!(rendered.contains(
        "%async_body_frame_field0 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 0"
    ));
    assert!(rendered.contains(
        "%async_body_frame_field1 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 1"
    ));
}

#[test]
fn emits_async_task_create_wrapper_with_recursive_aggregate_frame_fields() {
    let runtime_hooks = collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
return pair.right + values[1]
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i64 @ql_1_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
    assert!(rendered.contains(
        "getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %async_frame, i32 0, i32 0"
    ));
    assert!(rendered.contains(
        "getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %async_frame, i32 0, i32 1"
    ));
    assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
    assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
    assert!(
        rendered.contains(
            "%async_body_frame_field0 = getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %frame, i32 0, i32 0"
        )
    );
    assert!(
        rendered.contains(
            "%async_body_frame_field1 = getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %frame, i32 0, i32 1"
        )
    );
}

#[test]
fn emits_async_task_create_wrapper_with_zero_sized_recursive_aggregate_frame_fields() {
    let runtime_hooks = collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
return 0
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i64 @ql_1_worker__async_body(ptr %frame)"));
    assert!(rendered.contains(
        "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
    ));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
    assert!(rendered.contains(
        "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 0"
    ));
    assert!(rendered.contains(
        "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 1"
    ));
    assert!(rendered.contains(
        "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 2"
    ));
    assert!(rendered.contains("store [0 x i64] %arg0, ptr %async_frame_field0"));
    assert!(rendered.contains("store { [0 x i64] } %arg1, ptr %async_frame_field1"));
    assert!(rendered.contains("store [1 x [0 x i64]] %arg2, ptr %async_frame_field2"));
    assert!(rendered.contains(
        "%async_body_frame_field0 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 0"
    ));
    assert!(rendered.contains(
        "%async_body_frame_field1 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 1"
    ));
    assert!(rendered.contains(
        "%async_body_frame_field2 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 2"
    ));
}

#[test]
fn builds_async_task_result_layouts_for_void_scalar_string_task_handle_tuple_and_array_results() {
    let void_layout =
        build_async_task_result_layout(&Ty::Builtin(BuiltinType::Void), Span::new(0, 0))
            .expect("void async result layout should be supported");
    assert!(matches!(void_layout, AsyncTaskResultLayout::Void));
    assert_eq!(void_layout.body_llvm_ty(), "void");

    let int_layout =
        build_async_task_result_layout(&Ty::Builtin(BuiltinType::Int), Span::new(0, 0))
            .expect("scalar async result layout should be supported");
    match int_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "i64");
            assert_eq!(_size, 8);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => panic!("expected scalar layout for Int"),
    }

    let string_layout =
        build_async_task_result_layout(&Ty::Builtin(BuiltinType::String), Span::new(0, 0))
            .expect("string async result layout should be supported");
    match string_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "{ ptr, i64 }");
            assert_eq!(_size, 16);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => panic!("expected loadable layout for String"),
    }

    let task_handle_layout = build_async_task_result_layout(
        &Ty::TaskHandle(Box::new(Ty::Builtin(BuiltinType::Int))),
        Span::new(0, 0),
    )
    .expect("task-handle async result layout should be supported");
    match task_handle_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "ptr");
            assert_eq!(_size, 8);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => {
            panic!("expected loadable layout for task-handle result")
        }
    }

    let tuple_layout = build_async_task_result_layout(
        &Ty::Tuple(vec![
            Ty::Builtin(BuiltinType::Bool),
            Ty::Builtin(BuiltinType::Int),
        ]),
        Span::new(0, 0),
    )
    .expect("tuple async result layout should be supported");
    match tuple_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "{ i1, i64 }");
            assert_eq!(_size, 16);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => panic!("expected loadable layout for tuple result"),
    }

    let array_layout = build_async_task_result_layout(
        &Ty::Array {
            element: Box::new(Ty::Builtin(BuiltinType::Int)),
            len: TyArrayLen::Known(3),
        },
        Span::new(0, 0),
    )
    .expect("array async result layout should be supported");
    match array_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "[3 x i64]");
            assert_eq!(_size, 24);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => panic!("expected loadable layout for array result"),
    }
}

#[test]
fn builds_async_task_result_layouts_for_zero_sized_arrays() {
    let array_layout = build_async_task_result_layout(
        &Ty::Array {
            element: Box::new(Ty::Builtin(BuiltinType::Int)),
            len: TyArrayLen::Known(0),
        },
        Span::new(0, 0),
    )
    .expect("zero-sized array async result layout should be supported");
    match array_layout {
        AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size,
            align,
        } => {
            assert_eq!(llvm_ty, "[0 x i64]");
            assert_eq!(_size, 0);
            assert_eq!(align, 8);
        }
        AsyncTaskResultLayout::Void => {
            panic!("expected loadable layout for zero-sized array result")
        }
    }
}

#[test]
fn emits_scalar_struct_value_lowering_in_declaration_order() {
    let rendered = emit_library(
        r#"
struct Pair {
left: Bool,
right: Int,
}

fn pair() -> Pair {
return Pair { right: 42, left: true }
}
"#,
    );

    assert!(rendered.contains("define { i1, i64 } @ql_1_pair()"));
    assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
    assert!(rendered.contains("insertvalue { i1, i64 }"));
    assert!(rendered.contains("i64 42, 1"));
    assert!(rendered.contains("ret { i1, i64 }"));
}

#[test]
fn emits_fixed_array_value_lowering() {
    let rendered = emit_library(
        r#"
fn values() -> [Int; 3] {
return [1, 2, 3]
}
"#,
    );

    assert!(rendered.contains("define [3 x i64] @ql_0_values()"));
    assert!(rendered.contains("insertvalue [3 x i64] undef, i64 1, 0"));
    assert!(rendered.contains("i64 2, 1"));
    assert!(rendered.contains("i64 3, 2"));
    assert!(rendered.contains("ret [3 x i64]"));
}

#[test]
fn emits_repeat_array_value_lowering() {
    let rendered = emit_library(
        r#"
fn values(seed: Int) -> [Int; 3] {
return [seed + 1; 3]
}
"#,
    );

    assert!(rendered.contains("define [3 x i64] @ql_0_values(i64 %arg0)"));
    assert!(rendered.matches("insertvalue [3 x i64]").count() >= 3);
    assert!(rendered.contains("ret [3 x i64]"));
}

#[test]
fn emits_empty_array_value_lowering_when_return_type_is_known() {
    let rendered = emit_library(
        r#"
fn values() -> [Int; 0] {
return []
}
"#,
    );

    assert!(
        rendered.contains("define [0 x i64] @ql_0_values()"),
        "{rendered}"
    );
    assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
    assert!(rendered.contains("ret [0 x i64]"), "{rendered}");
}

#[test]
fn emits_empty_array_argument_lowering_when_callee_param_type_is_known() {
    let rendered = emit_library(
        r#"
fn take(values: [Int; 0]) -> Int {
return 0
}

fn call() -> Int {
return take([])
}
"#,
    );

    assert!(
        rendered.contains("define i64 @ql_0_take([0 x i64] %arg0)"),
        "{rendered}"
    );
    assert!(
        rendered.contains("call i64 @ql_0_take(") && rendered.contains("[0 x i64]"),
        "{rendered}"
    );
}

#[test]
fn emits_empty_array_lowering_inside_expected_tuple_items() {
    let rendered = emit_library(
        r#"
fn pair() -> ([Int; 0], Int) {
return ([], 1)
}
"#,
    );

    assert!(
        rendered.contains("define { [0 x i64], i64 } @ql_0_pair()"),
        "{rendered}"
    );
    assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
    assert!(
        rendered.contains("insertvalue { [0 x i64], i64 }"),
        "{rendered}"
    );
}

#[test]
fn emits_empty_array_lowering_inside_expected_struct_fields() {
    let rendered = emit_library(
        r#"
struct Wrap {
values: [Int; 0],
}

fn build() -> Wrap {
return Wrap { values: [] }
}
"#,
    );

    assert!(
        rendered.contains("define { [0 x i64] } @ql_1_build()"),
        "{rendered}"
    );
    assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
    assert!(rendered.contains("insertvalue { [0 x i64] }"), "{rendered}");
}

#[test]
fn emits_empty_array_lowering_inside_expected_nested_arrays() {
    let rendered = emit_library(
        r#"
fn values() -> [[Int; 0]; 1] {
return [[]]
}
"#,
    );

    assert!(
        rendered.contains("define [1 x [0 x i64]] @ql_0_values()"),
        "{rendered}"
    );
    assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
    assert!(
        rendered.contains("insertvalue [1 x [0 x i64]]"),
        "{rendered}"
    );
}

#[test]
fn rejects_empty_array_value_without_expected_type() {
    let messages = emit_error(
        r#"
fn main() -> Int {
let values = []
return 0
}
"#,
    );

    assert!(messages.iter().any(|message| {
        message
            == "LLVM IR backend foundation cannot infer the element type of an empty array literal without an expected array type"
    }));
}

#[test]
fn emits_struct_literal_lowering_through_same_file_import_alias() {
    let rendered = emit_library(
        r#"
use Pair as P

struct Pair {
left: Bool,
right: Int,
}

fn pair() -> Pair {
return P { right: 42, left: true }
}
"#,
    );

    assert!(rendered.contains("define { i1, i64 } @ql_1_pair()"));
    assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
    assert!(rendered.contains("i64 42, 1"));
}

#[test]
fn emits_struct_field_projection_reads() {
    let rendered = emit_library(
        r#"
struct Pair {
left: Bool,
right: Int,
}

fn right(pair: Pair) -> Int {
return pair.right
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_1_right({ i1, i64 } %arg0)"));
    assert!(rendered.contains("getelementptr inbounds { i1, i64 }, ptr"));
    assert!(rendered.contains("i32 0, i32 1"));
    assert!(rendered.contains("load i64, ptr"));
}

#[test]
fn emits_tuple_index_projection_reads() {
    let rendered = emit_library(
        r#"
fn second(pair: (Bool, Int)) -> Int {
return pair[1]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_second({ i1, i64 } %arg0)"));
    assert!(rendered.contains("getelementptr inbounds { i1, i64 }, ptr"));
    assert!(rendered.contains("i32 0, i32 1"));
    assert!(rendered.contains("load i64, ptr"));
}

#[test]
fn emits_array_index_projection_reads() {
    let rendered = emit_library(
        r#"
fn pick(values: [Int; 3], index: Int) -> Int {
return values[index]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_pick([3 x i64] %arg0, i64 %arg1)"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("i64 0, i64 %t"));
    assert!(rendered.contains("load i64, ptr"));
}

#[test]
fn emits_nested_projection_reads_through_recursive_aggregates() {
    let rendered = emit_library(
        r#"
struct Pair {
left: Int,
right: Int,
}

struct Outer {
pair: Pair,
values: [Int; 2],
}

fn pick_pair(outer: Outer) -> Int {
return outer.pair.right
}

fn pick_array(outer: Outer, index: Int) -> Int {
return outer.values[index]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_2_pick_pair({ { i64, i64 }, [2 x i64] } %arg0)"));
    assert!(rendered.contains("getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
    assert!(rendered.contains("getelementptr inbounds [2 x i64], ptr"));
}

#[test]
fn emits_struct_field_projection_writes() {
    let rendered = emit_library(
        r#"
struct Pair {
left: Int,
right: Int,
}

fn write_right() -> Int {
var pair = Pair { left: 1, right: 2 }
pair.right = 7
return pair.right
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_1_write_right()"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
    assert!(rendered.contains("i32 0, i32 1"));
    assert!(rendered.contains("store i64 7, ptr %t"));
}

#[test]
fn emits_tuple_index_projection_writes() {
    let rendered = emit_library(
        r#"
fn write_first() -> Int {
var pair = (1, 2)
pair[0] = 9
return pair[0]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_write_first()"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
    assert!(rendered.contains("i32 0, i32 0"));
    assert!(rendered.contains("store i64 9, ptr %t"));
}

#[test]
fn emits_array_index_projection_writes() {
    let rendered = emit_library(
        r#"
fn write_first() -> Int {
var values = [1, 2, 3]
values[0] = 9
return values[0]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_write_first()"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("i64 0, i64 0"));
    assert!(rendered.contains("store i64 9, ptr %t"));
}

#[test]
fn emits_dynamic_array_index_projection_writes() {
    let rendered = emit_library(
        r#"
fn write_at(index: Int) -> Int {
var values = [1, 2, 3]
values[index] = 9
return values[index]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_write_at(i64 %arg0)"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("i64 0, i64 %t"));
    assert!(rendered.contains("store i64 9, ptr %t"));
}

#[test]
fn emits_nested_dynamic_array_index_projection_writes() {
    let rendered = emit_library(
        r#"
fn write_cell(row: Int, col: Int) -> Int {
var matrix = [[1, 2, 3], [4, 5, 6]]
matrix[row][col] = 9
return matrix[row][col]
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_write_cell(i64 %arg0, i64 %arg1)"));
    assert!(rendered.contains("getelementptr inbounds [2 x [3 x i64]], ptr"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("store i64 9, ptr %t"));
}

#[test]
fn emits_dynamic_task_handle_array_index_projection_writes() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
var tasks = [worker(), worker()]
tasks[index] = worker()
return await tasks[0]
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.contains("i64 0, i64 %t"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn rejects_parameterized_async_function_bodies_without_async_frame_alloc_hook() {
    let messages = emit_error_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}
"#,
        CodegenMode::Library,
        &[runtime_hook_signature(RuntimeHook::AsyncTaskCreate)],
    );

    assert!(messages.iter().any(|message| {
        message
            == "LLVM IR backend foundation requires the `async-frame-alloc` runtime hook before lowering parameterized `async fn` bodies"
    }));
}

#[test]
fn rejects_async_function_bodies_without_async_task_create_hook() {
    let messages = emit_error_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}
"#,
        CodegenMode::Library,
        &[],
    );

    assert!(messages.iter().any(|message| {
        message
            == "LLVM IR backend foundation requires the `async-task-create` runtime hook before lowering `async fn` bodies"
    }));
}

#[test]
fn emits_async_struct_task_result_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Bool,
right: Int,
}

async fn worker() -> Pair {
return Pair { right: 42, left: true }
}

async fn helper() -> Pair {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define { i1, i64 } @ql_1_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
    assert!(rendered.contains("i64 42, 1"));
    assert!(rendered.contains("define { i1, i64 } @ql_2_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load { i1, i64 }, ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_async_array_task_result_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> [Int; 3] {
return [1, 2, 3]
}

async fn helper() -> [Int; 3] {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define [3 x i64] @ql_0_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("insertvalue [3 x i64] undef, i64 1, 0"));
    assert!(rendered.contains("i64 2, 1"));
    assert!(rendered.contains("i64 3, 2"));
    assert!(rendered.contains("define [3 x i64] @ql_1_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load [3 x i64], ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_async_zero_sized_array_task_result_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> [Int; 0] {
return []
}

async fn helper() -> [Int; 0] {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define [0 x i64] @ql_0_worker__async_body(ptr %frame)"));
    assert!(
        rendered.contains("store [0 x i64] zeroinitializer"),
        "{rendered}"
    );
    assert!(rendered.contains("define [0 x i64] @ql_1_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load [0 x i64], ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_async_zero_sized_recursive_aggregate_task_result_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define { [0 x i64] } @ql_1_worker__async_body(ptr %frame)"));
    assert!(
        rendered.contains("store [0 x i64] zeroinitializer"),
        "{rendered}"
    );
    assert!(rendered.contains("define { [0 x i64] } @ql_2_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_async_zero_sized_recursive_aggregate_param_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
return 7
}

async fn helper() -> Int {
return await worker([], Wrap { values: [] }, [[]])
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains(
        "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
    ));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
    assert!(rendered.contains("call ptr @ql_1_worker("));
    assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
    assert!(rendered.contains("[1 x [0 x i64]]"), "{rendered}");
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load i64, ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_async_recursive_aggregate_task_result_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
return (Pair { left: 1, right: 2 }, [3, 4])
}

async fn helper() -> (Pair, [Int; 2]) {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(
        rendered
            .contains("define { { i64, i64 }, [2 x i64] } @ql_1_worker__async_body(ptr %frame)")
    );
    assert!(rendered.contains("insertvalue { i64, i64 } undef, i64 1, 0"));
    assert!(rendered.contains("insertvalue [2 x i64] undef, i64 3, 0"));
    assert!(
        rendered
            .contains("define { { i64, i64 }, [2 x i64] } @ql_2_helper__async_body(ptr %frame)")
    );
    assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_scalar_tuple_value_lowering() {
    let rendered = emit_library(
        r#"
fn pair() -> (Bool, Int) {
return (true, 42)
}
"#,
    );

    assert!(rendered.contains("define { i1, i64 } @ql_0_pair()"));
    assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
    assert!(rendered.contains("i64 42, 1"));
    assert!(rendered.contains("ret { i1, i64 }"));
}

#[test]
fn library_mode_keeps_extern_block_declarations_for_direct_calls() {
    let rendered = emit_library(
        r#"
extern "c" {
fn q_add(left: Int, right: Int) -> Int
}

fn add_two(value: Int) -> Int {
return q_add(value, 2)
}
"#,
    );

    assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
    assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
    assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
}

#[test]
fn library_mode_keeps_top_level_extern_declarations_for_direct_calls() {
    let rendered = emit_library(
        r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn add_two(value: Int) -> Int {
return q_add(value, 2)
}
"#,
    );

    assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
    assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
    assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
}

#[test]
fn emits_extern_c_function_definitions_with_stable_symbol_names() {
    let rendered = emit(
        r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
return left + right
}

fn main() -> Int {
return q_add(1, 2)
}
"#,
    );

    assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
    assert!(rendered.contains("define i64 @ql_1_main()"));
    assert!(rendered.contains("call i64 @q_add(i64 1, i64 2)"));
    assert!(!rendered.contains("define i64 @ql_0_q_add"));
}

#[test]
fn library_mode_keeps_extern_c_function_definitions_exported() {
    let rendered = emit_library(
        r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
return left + right
}

fn add_two(value: Int) -> Int {
return q_add(value, 2)
}
"#,
    );

    assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
    assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
    assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
    assert!(!rendered.contains("define i64 @main("));
}

#[test]
fn rejects_non_c_extern_declarations() {
    let messages = emit_error(
        r#"
extern "rust" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
return q_add(1, 2)
}
"#,
    );

    assert!(messages.iter().any(|message| {
        message == "LLVM IR backend foundation only supports `extern \"c\"` declarations yet"
    }));
}

#[test]
fn rejects_non_c_extern_definitions() {
    let messages = emit_error(
        r#"
extern "rust" pub fn q_add(left: Int, right: Int) -> Int {
return left + right
}

fn main() -> Int {
return q_add(1, 2)
}
"#,
    );

    assert!(messages.iter().any(|message| {
        message
            == "LLVM IR backend foundation only supports `extern \"c\"` function definitions yet"
    }));
}

#[test]
fn rejects_extern_c_entry_main_definitions() {
    let messages = emit_error(
        r#"
extern "c" fn main() -> Int {
return 0
}
"#,
    );

    assert!(messages.iter().any(|message| {
        message
            == "entry function `main` must use the default Qlang ABI in the current native build pipeline"
    }));
}
