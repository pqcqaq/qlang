    use super::{
        CHeaderError, CHeaderOptions, CHeaderSurface, default_c_header_output_path,
        default_c_header_output_path_for_surface, emit_c_header, exported_c_symbol_names,
    };
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn default_c_header_output_path_uses_target_ql_ffi_layout() {
        let output =
            default_c_header_output_path(Path::new("D:/workspace/demo"), Path::new("src/app.ql"));

        assert_eq!(
            output,
            PathBuf::from("D:/workspace/demo/target/ql/ffi/app.h")
        );
    }

    #[test]
    fn default_c_header_output_path_for_surface_uses_surface_specific_suffixes() {
        let imports = default_c_header_output_path_for_surface(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            CHeaderSurface::Imports,
        );
        let both = default_c_header_output_path_for_surface(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            CHeaderSurface::Both,
        );

        assert_eq!(
            imports,
            PathBuf::from("D:/workspace/demo/target/ql/ffi/app.imports.h")
        );
        assert_eq!(
            both,
            PathBuf::from("D:/workspace/demo/target/ql/ffi/app.ffi.h")
        );
    }

    #[test]
    fn emit_c_header_writes_public_exported_extern_c_definitions() {
        let dir = TestDir::new("ql-driver-ffi-header");
        let source = dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

extern "c" fn imported_only(value: Int) -> Int

fn internal(value: Int) -> Int {
    return value
}
"#,
        );
        let output = dir.path().join("artifacts/ffi_export.h");

        let artifact = emit_c_header(
            &source,
            &CHeaderOptions {
                output: Some(output.clone()),
                ..CHeaderOptions::default()
            },
        )
        .expect("header generation should succeed");

        assert_eq!(artifact.path, output);
        assert_eq!(artifact.surface, CHeaderSurface::Exports);
        assert_eq!(artifact.exported_functions, 1);
        assert_eq!(artifact.imported_functions, 0);
        let rendered = fs::read_to_string(output).expect("read generated header");
        assert_eq!(
            rendered,
            "\
#ifndef QLANG_FFI_EXPORT_H\n\
#define QLANG_FFI_EXPORT_H\n\
\n\
#include <stdbool.h>\n\
#include <stdint.h>\n\
\n\
#ifdef __cplusplus\n\
extern \"C\" {\n\
#endif\n\
\n\
int64_t q_add(int64_t left, int64_t right);\n\
\n\
#ifdef __cplusplus\n\
}\n\
#endif\n\
\n\
#endif /* QLANG_FFI_EXPORT_H */\n"
        );
    }

    #[test]
    fn emit_c_header_writes_imported_extern_c_declarations_and_extern_block_members() {
        let dir = TestDir::new("ql-driver-ffi-import-header");
        let source = dir.write(
            "ffi_imports.ql",
            r#"
extern "c" fn q_host_log(message: *const U8) -> Void

extern "c" {
    fn q_host_add(left: Int, right: Int) -> Int
}

extern "c" pub fn q_exported(value: Int) -> Int {
    return value
}
"#,
        );
        let output = dir.path().join("artifacts/ffi_imports.h");

        let artifact = emit_c_header(
            &source,
            &CHeaderOptions {
                output: Some(output.clone()),
                surface: CHeaderSurface::Imports,
            },
        )
        .expect("import header generation should succeed");

        assert_eq!(artifact.path, output);
        assert_eq!(artifact.surface, CHeaderSurface::Imports);
        assert_eq!(artifact.exported_functions, 0);
        assert_eq!(artifact.imported_functions, 2);
        let rendered = fs::read_to_string(output).expect("read generated import header");
        assert_eq!(
            rendered,
            "\
#ifndef QLANG_FFI_IMPORTS_H\n\
#define QLANG_FFI_IMPORTS_H\n\
\n\
#include <stdbool.h>\n\
#include <stdint.h>\n\
\n\
#ifdef __cplusplus\n\
extern \"C\" {\n\
#endif\n\
\n\
void q_host_log(const uint8_t* message);\n\
int64_t q_host_add(int64_t left, int64_t right);\n\
\n\
#ifdef __cplusplus\n\
}\n\
#endif\n\
\n\
#endif /* QLANG_FFI_IMPORTS_H */\n"
        );
    }

    #[test]
    fn emit_c_header_writes_combined_import_and_export_surface() {
        let dir = TestDir::new("ql-driver-ffi-both-header");
        let source = dir.write(
            "ffi_surface.ql",
            r#"
extern "c" fn q_host_log(message: *const U8) -> Void

extern "c" {
    fn q_host_add(left: Int, right: Int) -> Int
}

extern "c" pub fn q_exported(value: Int) -> Int {
    return value
}
"#,
        );
        let output = dir.path().join("artifacts/ffi_surface.ffi.h");

        let artifact = emit_c_header(
            &source,
            &CHeaderOptions {
                output: Some(output.clone()),
                surface: CHeaderSurface::Both,
            },
        )
        .expect("combined header generation should succeed");

        assert_eq!(artifact.path, output);
        assert_eq!(artifact.surface, CHeaderSurface::Both);
        assert_eq!(artifact.exported_functions, 1);
        assert_eq!(artifact.imported_functions, 2);
        let rendered = fs::read_to_string(output).expect("read generated combined header");
        assert_eq!(
            rendered,
            "\
#ifndef QLANG_FFI_SURFACE_FFI_H\n\
#define QLANG_FFI_SURFACE_FFI_H\n\
\n\
#include <stdbool.h>\n\
#include <stdint.h>\n\
\n\
#ifdef __cplusplus\n\
extern \"C\" {\n\
#endif\n\
\n\
void q_host_log(const uint8_t* message);\n\
int64_t q_host_add(int64_t left, int64_t right);\n\
int64_t q_exported(int64_t value);\n\
\n\
#ifdef __cplusplus\n\
}\n\
#endif\n\
\n\
#endif /* QLANG_FFI_SURFACE_FFI_H */\n"
        );
    }

    #[test]
    fn emit_c_header_supports_pointer_exports() {
        let dir = TestDir::new("ql-driver-ffi-pointer-header");
        let source = dir.write(
            "ffi_pointer.ql",
            r#"
extern "c" pub fn fill(buf: *U8, src: *const U8) -> *const U8 {
    return src
}
"#,
        );
        let output = dir.path().join("artifacts/ffi_pointer.h");

        emit_c_header(
            &source,
            &CHeaderOptions {
                output: Some(output.clone()),
                ..CHeaderOptions::default()
            },
        )
        .expect("pointer header generation should succeed");

        let rendered = fs::read_to_string(output).expect("read generated pointer header");
        assert!(rendered.contains("uint8_t* buf"));
        assert!(rendered.contains("const uint8_t* src"));
        assert!(rendered.contains("const uint8_t* fill"));
    }

    #[test]
    fn emit_c_header_supports_string_exports() {
        let dir = TestDir::new("ql-driver-ffi-string-header");
        let source = dir.write(
            "ffi_string.ql",
            r#"
extern "c" pub fn q_echo(message: String) -> String {
    return message
}
"#,
        );
        let output = dir.path().join("artifacts/ffi_string.h");

        emit_c_header(
            &source,
            &CHeaderOptions {
                output: Some(output.clone()),
                ..CHeaderOptions::default()
            },
        )
        .expect("string header generation should succeed");

        let rendered = fs::read_to_string(output).expect("read generated string header");
        assert_eq!(
            rendered,
            concat!(
                "#ifndef QLANG_FFI_STRING_H\n",
                "#define QLANG_FFI_STRING_H\n",
                "\n",
                "#include <stdbool.h>\n",
                "#include <stdint.h>\n",
                "\n",
                "typedef struct ql_string {\n",
                "    const uint8_t* ptr;\n",
                "    int64_t len;\n",
                "} ql_string;\n",
                "\n",
                "#ifdef __cplusplus\n",
                "extern \"C\" {\n",
                "#endif\n",
                "\n",
                "ql_string q_echo(ql_string message);\n",
                "\n",
                "#ifdef __cplusplus\n",
                "}\n",
                "#endif\n",
                "\n",
                "#endif /* QLANG_FFI_STRING_H */\n"
            )
        );
    }

    #[test]
    fn emit_c_header_requires_at_least_one_public_export() {
        let dir = TestDir::new("ql-driver-ffi-header-empty");
        let source = dir.write(
            "ffi_empty.ql",
            r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );

        let error = emit_c_header(&source, &CHeaderOptions::default())
            .expect_err("header generation should require a public export");

        match error {
            CHeaderError::InvalidInput(message) => assert!(
                message.contains("does not define any public exported `extern \"c\"` functions")
            ),
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    #[test]
    fn exported_c_symbol_names_only_collect_public_definitions() {
        let module = ql_analysis::analyze_source(
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

extern "c" fn q_hidden(left: Int, right: Int) -> Int {
    return left + right
}

extern "c" pub fn q_imported(left: Int, right: Int) -> Int

fn q_internal() -> Int {
    return 0
}
"#,
        )
        .expect("analysis should succeed");

        assert_eq!(exported_c_symbol_names(module.hir()), vec!["q_add"]);
    }
