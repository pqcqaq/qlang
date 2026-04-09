use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ql_analysis::{Analysis, analyze_source};
use ql_ast::Visibility;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{self as hir, ItemKind, Param};
use ql_resolve::{BuiltinType, ResolutionMap};
use ql_typeck::{Ty, lower_type};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CHeaderSurface {
    #[default]
    Exports,
    Imports,
    Both,
}

impl CHeaderSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exports => "exports",
            Self::Imports => "imports",
            Self::Both => "both",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exports" => Some(Self::Exports),
            "imports" => Some(Self::Imports),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub const fn includes_exports(self) -> bool {
        matches!(self, Self::Exports | Self::Both)
    }

    pub const fn includes_imports(self) -> bool {
        matches!(self, Self::Imports | Self::Both)
    }

    pub(crate) const fn output_suffix(self) -> Option<&'static str> {
        match self {
            Self::Exports => None,
            Self::Imports => Some("imports"),
            Self::Both => Some("ffi"),
        }
    }

    fn empty_surface_description(self) -> &'static str {
        match self {
            Self::Exports => "any public exported `extern \"c\"` functions with bodies",
            Self::Imports => "any imported `extern \"c\"` function declarations",
            Self::Both => "any imported or exported `extern \"c\"` functions",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CHeaderOptions {
    pub output: Option<PathBuf>,
    pub surface: CHeaderSurface,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CHeaderArtifact {
    pub path: PathBuf,
    pub surface: CHeaderSurface,
    pub exported_functions: usize,
    pub imported_functions: usize,
}

#[derive(Debug)]
pub enum CHeaderError {
    InvalidInput(String),
    Io {
        path: PathBuf,
        error: io::Error,
    },
    Diagnostics {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
}

impl CHeaderError {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::InvalidInput(_) => None,
            Self::Io { path, .. } | Self::Diagnostics { path, .. } => Some(path),
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Diagnostics { source, .. } => Some(source),
            Self::InvalidInput(_) | Self::Io { .. } => None,
        }
    }

    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        match self {
            Self::Diagnostics { diagnostics, .. } => Some(diagnostics),
            Self::InvalidInput(_) | Self::Io { .. } => None,
        }
    }
}

pub fn emit_c_header(
    path: &Path,
    options: &CHeaderOptions,
) -> Result<CHeaderArtifact, CHeaderError> {
    if !path.is_file() {
        return Err(CHeaderError::InvalidInput(format!(
            "`{}` is not a file",
            path.display()
        )));
    }

    let source = fs::read_to_string(path).map_err(|error| CHeaderError::Io {
        path: path.to_path_buf(),
        error,
    })?;

    let analysis = analyze_source(&source).map_err(|diagnostics| CHeaderError::Diagnostics {
        path: path.to_path_buf(),
        source: source.clone(),
        diagnostics,
    })?;

    emit_c_header_from_analysis(path, &source, &analysis, options)
}

pub(crate) fn emit_c_header_from_analysis(
    path: &Path,
    source: &str,
    analysis: &Analysis,
    options: &CHeaderOptions,
) -> Result<CHeaderArtifact, CHeaderError> {
    if analysis.has_errors() {
        return Err(CHeaderError::Diagnostics {
            path: path.to_path_buf(),
            source: source.to_owned(),
            diagnostics: analysis.diagnostics().to_vec(),
        });
    }

    let functions =
        collect_c_header_functions(analysis.hir(), analysis.resolution(), options.surface)
            .map_err(|diagnostics| CHeaderError::Diagnostics {
                path: path.to_path_buf(),
                source: source.to_owned(),
                diagnostics,
            })?;
    if functions.is_empty() {
        return Err(CHeaderError::InvalidInput(format!(
            "`{}` does not define {}",
            path.display(),
            options.surface.empty_surface_description()
        )));
    }

    let output_path = resolve_c_header_output_path(path, options)?;
    write_c_header_artifact(output_path, options.surface, &functions)
}

fn resolve_c_header_output_path(
    path: &Path,
    options: &CHeaderOptions,
) -> Result<PathBuf, CHeaderError> {
    let output_path = match &options.output {
        Some(path) => path.clone(),
        None => {
            let build_root = env::current_dir().map_err(|error| CHeaderError::Io {
                path: PathBuf::from("."),
                error,
            })?;
            default_c_header_output_path_for_surface(&build_root, path, options.surface)
        }
    };

    Ok(output_path)
}

fn write_c_header_artifact(
    output_path: PathBuf,
    surface: CHeaderSurface,
    functions: &[CHeaderFunction],
) -> Result<CHeaderArtifact, CHeaderError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CHeaderError::Io {
            path: parent.to_path_buf(),
            error,
        })?;
    }

    let rendered = render_c_header(&output_path, functions);
    fs::write(&output_path, rendered).map_err(|error| CHeaderError::Io {
        path: output_path.clone(),
        error,
    })?;

    let exported_functions = functions
        .iter()
        .filter(|function| function.kind == CHeaderFunctionKind::Export)
        .count();
    let imported_functions = functions
        .iter()
        .filter(|function| function.kind == CHeaderFunctionKind::Import)
        .count();

    Ok(CHeaderArtifact {
        path: output_path,
        surface,
        exported_functions,
        imported_functions,
    })
}

pub fn default_c_header_output_path(build_root: &Path, input_path: &Path) -> PathBuf {
    default_c_header_output_path_for_surface(build_root, input_path, CHeaderSurface::Exports)
}

pub fn default_c_header_output_path_for_surface(
    build_root: &Path,
    input_path: &Path,
    surface: CHeaderSurface,
) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface.output_suffix() {
        Some(suffix) => format!("{stem}.{suffix}.h"),
        None => format!("{stem}.h"),
    };
    build_root
        .join("target")
        .join("ql")
        .join("ffi")
        .join(file_name)
}

pub(crate) fn exported_c_symbol_names(module: &hir::Module) -> Vec<String> {
    module
        .items
        .iter()
        .filter_map(|item_id| {
            let ItemKind::Function(function) = &module.item(*item_id).kind else {
                return None;
            };
            is_exported_c_definition(function).then(|| function.name.clone())
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CHeaderFunction {
    kind: CHeaderFunctionKind,
    name: String,
    params: Vec<CHeaderParam>,
    return_ty: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CHeaderParam {
    name: String,
    ty: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CHeaderFunctionKind {
    Export,
    Import,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CTypeSpelling {
    base: String,
    pointer_constness: Vec<bool>,
}

fn collect_c_header_functions(
    module: &hir::Module,
    resolution: &ResolutionMap,
    surface: CHeaderSurface,
) -> Result<Vec<CHeaderFunction>, Vec<Diagnostic>> {
    let mut functions = Vec::new();
    let mut diagnostics = Vec::new();

    for item_id in &module.items {
        match &module.item(*item_id).kind {
            ItemKind::Function(function) => {
                let Some(kind) = classify_top_level_c_header_function(function, surface) else {
                    continue;
                };
                collect_c_header_function(
                    module,
                    resolution,
                    function,
                    kind,
                    &mut functions,
                    &mut diagnostics,
                );
            }
            ItemKind::ExternBlock(extern_block) => {
                if !surface.includes_imports() || extern_block.abi != "c" {
                    continue;
                }
                for function in &extern_block.functions {
                    collect_c_header_function(
                        module,
                        resolution,
                        function,
                        CHeaderFunctionKind::Import,
                        &mut functions,
                        &mut diagnostics,
                    );
                }
            }
            _ => {}
        }
    }

    if diagnostics.is_empty() {
        Ok(functions)
    } else {
        Err(diagnostics)
    }
}

fn collect_c_header_function(
    module: &hir::Module,
    resolution: &ResolutionMap,
    function: &hir::Function,
    kind: CHeaderFunctionKind,
    functions: &mut Vec<CHeaderFunction>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let role = c_header_function_role(kind);

    if !function.generics.is_empty() {
        diagnostics.push(unsupported_c_header_function(
            function.span,
            format!("C header generation does not support generic {role} functions yet"),
        ));
        return;
    }
    if !function.where_clause.is_empty() {
        diagnostics.push(unsupported_c_header_function(
            function.span,
            format!("C header generation does not support `where` clauses on {role} functions yet"),
        ));
        return;
    }
    if function.is_async {
        diagnostics.push(unsupported_c_header_function(
            function.span,
            format!("C header generation does not support `async fn` {role} functions yet"),
        ));
        return;
    }
    if function.is_unsafe {
        diagnostics.push(unsupported_c_header_function(
            function.span,
            format!("C header generation does not support `unsafe fn` {role} functions yet"),
        ));
        return;
    }

    let mut params = Vec::new();
    let mut function_has_errors = false;
    for param in &function.params {
        match param {
            Param::Regular(param) => {
                let ty = lower_type(module, resolution, param.ty);
                match render_c_type(&ty, module.ty(param.ty).span, "parameter type") {
                    Ok(spelling) => params.push(CHeaderParam {
                        name: param.name.clone(),
                        ty: spelling,
                    }),
                    Err(error) => {
                        function_has_errors = true;
                        diagnostics.push(error);
                    }
                }
            }
            Param::Receiver(receiver) => {
                function_has_errors = true;
                diagnostics.push(unsupported_c_header_function(
                    receiver.span,
                    format!(
                        "C header generation does not support receiver methods on {role} functions yet"
                    ),
                ));
            }
        }
    }

    let return_ty = match function.return_type {
        Some(type_id) => {
            let ty = lower_type(module, resolution, type_id);
            match render_c_type(&ty, module.ty(type_id).span, "return type") {
                Ok(spelling) => spelling,
                Err(error) => {
                    function_has_errors = true;
                    diagnostics.push(error);
                    String::new()
                }
            }
        }
        None => "void".to_owned(),
    };

    if function_has_errors {
        return;
    }

    functions.push(CHeaderFunction {
        kind,
        name: function.name.clone(),
        params,
        return_ty,
    });
}

fn classify_top_level_c_header_function(
    function: &hir::Function,
    surface: CHeaderSurface,
) -> Option<CHeaderFunctionKind> {
    if function.abi.as_deref() != Some("c") {
        return None;
    }

    match (
        function.body.is_some(),
        function.visibility == Visibility::Public,
    ) {
        (true, true) if surface.includes_exports() => Some(CHeaderFunctionKind::Export),
        (false, _) if surface.includes_imports() => Some(CHeaderFunctionKind::Import),
        _ => None,
    }
}

fn c_header_function_role(kind: CHeaderFunctionKind) -> &'static str {
    match kind {
        CHeaderFunctionKind::Export => "exported",
        CHeaderFunctionKind::Import => "imported",
    }
}

fn is_exported_c_definition(function: &hir::Function) -> bool {
    function.visibility == Visibility::Public
        && function.abi.as_deref() == Some("c")
        && function.body.is_some()
}

fn render_c_header(header_path: &Path, functions: &[CHeaderFunction]) -> String {
    let include_guard = include_guard_name(header_path);
    let mut output = String::new();

    output.push_str("#ifndef ");
    output.push_str(&include_guard);
    output.push('\n');
    output.push_str("#define ");
    output.push_str(&include_guard);
    output.push_str("\n\n");
    output.push_str("#include <stdbool.h>\n");
    output.push_str("#include <stdint.h>\n\n");
    if header_uses_string(functions) {
        output.push_str("typedef struct ql_string {\n");
        output.push_str("    const uint8_t* ptr;\n");
        output.push_str("    int64_t len;\n");
        output.push_str("} ql_string;\n\n");
    }
    output.push_str("#ifdef __cplusplus\n");
    output.push_str("extern \"C\" {\n");
    output.push_str("#endif\n\n");

    for function in functions {
        output.push_str(&function.return_ty);
        output.push(' ');
        output.push_str(&function.name);
        output.push('(');
        if function.params.is_empty() {
            output.push_str("void");
        } else {
            for (index, param) in function.params.iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                output.push_str(&param.ty);
                output.push(' ');
                output.push_str(&param.name);
            }
        }
        output.push_str(");\n");
    }

    output.push_str("\n#ifdef __cplusplus\n");
    output.push_str("}\n");
    output.push_str("#endif\n\n");
    output.push_str("#endif /* ");
    output.push_str(&include_guard);
    output.push_str(" */\n");
    output
}

fn header_uses_string(functions: &[CHeaderFunction]) -> bool {
    functions.iter().any(|function| {
        function.return_ty.contains("ql_string")
            || function
                .params
                .iter()
                .any(|param| param.ty.contains("ql_string"))
    })
}

fn render_c_type(ty: &Ty, span: ql_span::Span, context: &str) -> Result<String, Diagnostic> {
    let spelling = lower_c_type_spelling(ty, span, context)?;
    Ok(render_c_type_spelling(&spelling))
}

fn lower_c_type_spelling(
    ty: &Ty,
    span: ql_span::Span,
    context: &str,
) -> Result<CTypeSpelling, Diagnostic> {
    match ty {
        Ty::Builtin(BuiltinType::Bool) => Ok(CTypeSpelling {
            base: "bool".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::Void) => Ok(CTypeSpelling {
            base: "void".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::Int)
        | Ty::Builtin(BuiltinType::I64)
        | Ty::Builtin(BuiltinType::ISize) => Ok(CTypeSpelling {
            base: "int64_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::UInt)
        | Ty::Builtin(BuiltinType::U64)
        | Ty::Builtin(BuiltinType::USize) => Ok(CTypeSpelling {
            base: "uint64_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::I32) => Ok(CTypeSpelling {
            base: "int32_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::U32) => Ok(CTypeSpelling {
            base: "uint32_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::I16) => Ok(CTypeSpelling {
            base: "int16_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::U16) => Ok(CTypeSpelling {
            base: "uint16_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::I8) => Ok(CTypeSpelling {
            base: "int8_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::U8) => Ok(CTypeSpelling {
            base: "uint8_t".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::F32) => Ok(CTypeSpelling {
            base: "float".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::F64) => Ok(CTypeSpelling {
            base: "double".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Builtin(BuiltinType::String) => Ok(CTypeSpelling {
            base: "ql_string".to_owned(),
            pointer_constness: Vec::new(),
        }),
        Ty::Pointer { is_const, inner } => {
            let mut inner = lower_c_type_spelling(inner, span, context)?;
            inner.pointer_constness.push(*is_const);
            Ok(inner)
        }
        _ => Err(Diagnostic::error(format!(
            "C header generation does not support {context} `{ty}` yet"
        ))
        .with_label(Label::new(span))),
    }
}

fn render_c_type_spelling(spelling: &CTypeSpelling) -> String {
    let mut rendered = spelling.base.clone();

    for (index, is_const) in spelling.pointer_constness.iter().copied().enumerate() {
        if is_const {
            if index == 0 && !rendered.contains('*') {
                rendered = format!("const {rendered}*");
            } else {
                rendered = format!("{rendered} const*");
            }
        } else {
            rendered.push('*');
        }
    }

    rendered
}

fn include_guard_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    format!("QLANG_{}_H", sanitize_macro_segment(stem))
}

fn sanitize_macro_segment(raw: &str) -> String {
    let mut output = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_uppercase());
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "MODULE".to_owned()
    } else {
        output
    }
}

fn unsupported_c_header_function(span: ql_span::Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message).with_label(Label::new(span))
}

#[cfg(test)]
mod tests {
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
}
