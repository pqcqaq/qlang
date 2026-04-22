    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ql_driver::{
        ArchiverFlavor, ArchiverInvocation, BuildEmit, BuildOptions, BuildProfile,
        ProgramInvocation, ToolchainOptions,
    };

    use super::{
        ProjectTargetSelector, analyze_semantics, analyze_source, build_path, collect_ql_files,
        dependency_public_struct_method_bridge_candidates,
        dependency_public_type_bridge_candidates, dependency_public_type_bridge_order,
        parse_source, render_mir_path, render_ownership_path, render_runtime_requirements,
    };

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create test parent directory");
            }
            fs::write(&path, contents).expect("write test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn relative_paths(root: &Path, files: Vec<PathBuf>) -> Vec<String> {
        files
            .into_iter()
            .map(|path| {
                path.strip_prefix(root)
                    .expect("file should be under test root")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn collect_ql_files_skips_tooling_and_negative_fixture_dirs() {
        let dir = TestDir::new("ql-cli-scan");
        dir.write("src/main.ql", "fn main() {}");
        dir.write("fixtures/parser/pass/good.ql", "fn good() {}");
        dir.write("fixtures/parser/fail/bad.ql", "fn");
        dir.write("ramdon_tests/scratch.ql", "fn scratch() {}");
        dir.write("target/generated.ql", "fn generated() {}");
        dir.write("node_modules/pkg/index.ql", "fn dep() {}");
        dir.write(".git/hooks/pre-commit.ql", "fn hook() {}");

        let files = collect_ql_files(dir.path()).expect("collect ql files");

        assert_eq!(relative_paths(dir.path(), files), vec!["src/main.ql"]);
    }

    #[test]
    fn collect_ql_files_respects_explicit_negative_fixture_roots() {
        let dir = TestDir::new("ql-cli-explicit-fail");
        dir.write("fixtures/parser/fail/bad.ql", "fn");

        let root = dir.path().join("fixtures/parser/fail");
        let files = collect_ql_files(&root).expect("collect explicit fail fixture files");

        assert_eq!(relative_paths(&root, files), vec!["bad.ql"]);
    }

    #[test]
    fn dependency_public_struct_method_bridge_candidates_include_trait_impl_methods() {
        let module = parse_source(
            r#"
pub trait Reader {
    fn read(self) -> Int
}

pub struct Box {
    value: Int,
}

impl Reader for Box {
    pub fn read(self) -> Int {
        return self.value
    }
}
"#,
        )
        .expect("source should parse");

        let methods = dependency_public_struct_method_bridge_candidates(&module, "Box");
        let read = methods
            .get("read")
            .expect("trait receiver method should be bridgeable");

        assert_eq!(methods.len(), 1);
        assert_eq!(read.name, "read");
        assert!(read.body.is_some());
    }

    #[test]
    fn dependency_public_type_bridge_order_supports_public_enum_payload_dependencies() {
        let module = parse_source(
            r#"
pub struct Issue {
    code: Int,
}

pub enum Status {
    Ready,
    Failed(Issue),
}
"#,
        )
        .expect("source should parse");

        let candidates = dependency_public_type_bridge_candidates(&module);
        let ordered = dependency_public_type_bridge_order("Status", &candidates)
            .expect("enum payload dependency order should resolve");

        assert_eq!(ordered, vec!["Issue".to_owned(), "Status".to_owned()]);
    }

    #[test]
    fn analyze_source_reports_semantic_errors() {
        let diagnostics = analyze_source(
            r#"
struct User {}
fn User() {}
"#,
        )
        .expect_err("source should have semantic diagnostics");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == "duplicate top-level definition `User`")
        );
    }

    #[test]
    fn analyze_source_reports_resolution_errors() {
        let diagnostics = analyze_source(
            r#"
fn main() -> Int {
    self
}
"#,
        )
        .expect_err("source should have resolver diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "invalid use of `self` outside a method receiver scope"
        }));
    }

    #[test]
    fn analyze_source_reports_type_errors() {
        let diagnostics = analyze_source(
            r#"
fn main() -> Int {
    return "oops"
}
"#,
        )
        .expect_err("source should have type diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "return value has type mismatch: expected `Int`, found `String`"
        }));
    }

    #[test]
    fn render_mir_path_succeeds_for_valid_sources() {
        let dir = TestDir::new("ql-cli-mir");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        );

        assert!(render_mir_path(&dir.path().join("sample.ql")).is_ok());
    }

    #[test]
    fn render_ownership_path_surfaces_ownership_reports() {
        let dir = TestDir::new("ql-cli-ownership");
        dir.write(
            "sample.ql",
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
        );

        let result = render_ownership_path(&dir.path().join("sample.ql"));
        assert!(
            result.is_err(),
            "ownership diagnostics should fail the command"
        );
    }

    #[test]
    fn render_runtime_requirements_reports_async_surface() {
        let analysis = analyze_semantics(
            r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    let task = spawn helper()
    return await helper()
}

async fn helper() -> Int {
    return 1
}
"#,
        )
        .expect("source should analyze");

        let rendered = render_runtime_requirements(&analysis);
        assert!(rendered.contains("runtime requirement: async-function-bodies @"));
        assert!(rendered.contains("runtime requirement: async-iteration @"));
        assert!(rendered.contains("runtime requirement: task-spawn @"));
        assert!(rendered.contains("runtime requirement: task-await @"));
        assert!(rendered.contains("runtime hook: async-frame-alloc -> qlrt_async_frame_alloc"));
        assert!(rendered.contains("runtime hook: async-task-create -> qlrt_async_task_create"));
        assert!(rendered.contains("runtime hook: executor-spawn -> qlrt_executor_spawn"));
        assert!(rendered.contains("runtime hook: task-await -> qlrt_task_await"));
        assert!(rendered.contains("runtime hook: task-result-release -> qlrt_task_result_release"));
        assert!(rendered.contains("runtime hook: async-iter-next -> qlrt_async_iter_next"));
        assert!(rendered.contains(
            "runtime hook abi: async-frame-alloc ccc qlrt_async_frame_alloc(size: i64, align: i64) -> ptr"
        ));
        assert!(rendered.contains(
            "runtime hook abi: async-task-create ccc qlrt_async_task_create(entry_fn: ptr, frame: ptr) -> ptr"
        ));
        assert!(rendered.contains(
            "runtime hook abi: executor-spawn ccc qlrt_executor_spawn(executor: ptr, task: ptr) -> ptr"
        ));
        assert!(
            rendered
                .contains("runtime hook abi: task-await ccc qlrt_task_await(handle: ptr) -> ptr")
        );
        assert!(rendered.contains(
            "runtime hook abi: task-result-release ccc qlrt_task_result_release(result: ptr) -> void"
        ));
        assert!(rendered.contains(
            "runtime hook abi: async-iter-next ccc qlrt_async_iter_next(iterator: ptr) -> ptr"
        ));
    }

    #[test]
    fn render_runtime_requirements_reports_none_for_sync_sources() {
        let analysis = analyze_semantics(
            r#"
fn main() -> Int {
    return 1
}
"#,
        )
        .expect("source should analyze");

        assert_eq!(
            render_runtime_requirements(&analysis),
            "runtime requirements: none\n"
        );
    }

    #[test]
    fn build_path_emits_llvm_ir_for_supported_source() {
        let dir = TestDir::new("ql-cli-build");
        dir.write(
            "sample.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    return add_one(41)
}
"#,
        );
        let output = dir.path().join("artifacts/sample.ll");
        let options = BuildOptions {
            emit: BuildEmit::LlvmIr,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions::default(),
        };

        assert!(
            build_path(
                &dir.path().join("sample.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered = fs::read_to_string(output).expect("read emitted LLVM IR");
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
    }

    #[test]
    fn build_path_emits_assembly_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-asm");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join("artifacts/sample.s");
        let options = BuildOptions {
            emit: BuildEmit::Assembly,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(
            build_path(
                &dir.path().join("sample.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered = fs::read_to_string(output).expect("read emitted assembly placeholder");
        assert_eq!(rendered, "mock-assembly");
    }

    #[test]
    fn build_path_emits_object_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-obj");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/sample.obj"
        } else {
            "artifacts/sample.o"
        });
        let options = BuildOptions {
            emit: BuildEmit::Object,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(
            build_path(
                &dir.path().join("sample.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered = fs::read_to_string(output).expect("read emitted object placeholder");
        assert_eq!(rendered, "mock-object");
    }

    #[test]
    fn build_path_emits_executable_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-exe");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/sample.exe"
        } else {
            "artifacts/sample"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(
            build_path(
                &dir.path().join("sample.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered = fs::read_to_string(output).expect("read emitted executable placeholder");
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_path_emits_dynamic_library_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-dylib");
        dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libffi_export.dylib"
        } else {
            "artifacts/libffi_export.so"
        });
        let options = BuildOptions {
            emit: BuildEmit::DynamicLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(
            build_path(
                &dir.path().join("ffi_export.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered =
            fs::read_to_string(output).expect("read emitted dynamic library placeholder");
        assert_eq!(rendered, "mock-dylib");
    }

    #[test]
    fn build_path_emits_static_library_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-staticlib");
        dir.write(
            "math.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/math.lib"
        } else {
            "artifacts/libmath.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        assert!(
            build_path(
                &dir.path().join("math.ql"),
                &options,
                &ProjectTargetSelector::default(),
                false,
                false,
                false,
                false,
            )
            .is_ok()
        );

        let rendered = fs::read_to_string(output).expect("read emitted static library placeholder");
        assert_eq!(rendered, "mock-staticlib");
    }

    fn mock_success_invocation(dir: &TestDir) -> ProgramInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-clang-success.ps1",
                r#"
$out = $null
$isCompile = $false
$isAssembly = $false
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-S') {
        $isAssembly = $true
    }
    if ($args[$i] -eq '-c') {
        $isCompile = $true
    }
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') {
        $isShared = $true
    }
    if ($args[$i] -eq '-o') {
        $out = $args[$i + 1]
    }
}
if ($null -eq $out) {
    Write-Error "missing -o"
    exit 1
}
if ($isAssembly) {
    Set-Content -Path $out -NoNewline -Value "mock-assembly"
} elseif ($isCompile) {
    Set-Content -Path $out -NoNewline -Value "mock-object"
} elseif ($isShared) {
    Set-Content -Path $out -NoNewline -Value "mock-dylib"
} else {
    Set-Content -Path $out -NoNewline -Value "mock-executable"
}
"#,
            );
            ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ])
        } else {
            let script = dir.write(
                "mock-clang-success.sh",
                r#"out=""
is_compile=0
is_assembly=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-S" ]; then
    is_assembly=1
    shift
    continue
  fi
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$is_assembly" -eq 1 ]; then
  printf 'mock-assembly' > "$out"
elif [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
            );
            ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
        }
    }

    fn mock_success_archiver_invocation(dir: &TestDir) -> ArchiverInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-archiver-success.ps1",
                r#"
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -like '/OUT:*') {
        $out = $args[$i].Substring(5)
    }
}
if ($null -eq $out) {
    Write-Error "missing /OUT"
    exit 1
}
Set-Content -Path $out -NoNewline -Value "mock-staticlib"
"#,
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                    "-ExecutionPolicy".to_owned(),
                    "Bypass".to_owned(),
                    "-File".to_owned(),
                    script.display().to_string(),
                ]),
                flavor: ArchiverFlavor::Lib,
            }
        } else {
            let script = dir.write(
                "mock-archiver-success.sh",
                r#"out="$2"
printf 'mock-staticlib' > "$out"
"#,
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("/bin/sh")
                    .with_args_prefix(vec![script.display().to_string()]),
                flavor: ArchiverFlavor::Ar,
            }
        }
    }
