use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramInvocation {
    pub program: PathBuf,
    pub args_prefix: Vec<String>,
}

impl ProgramInvocation {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args_prefix: Vec::new(),
        }
    }

    pub fn with_args_prefix<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args_prefix = args.into_iter().map(Into::into).collect();
        self
    }

    fn display_name(&self) -> String {
        self.program.display().to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiverFlavor {
    Ar,
    Lib,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiverInvocation {
    pub program: ProgramInvocation,
    pub flavor: ArchiverFlavor,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolchainOptions {
    pub clang: Option<ProgramInvocation>,
    pub archiver: Option<ArchiverInvocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredToolchain {
    clang: ProgramInvocation,
    archiver: Option<ArchiverInvocation>,
}

impl DiscoveredToolchain {
    pub fn ensure_archiver_available(&self) -> Result<(), ToolchainError> {
        if self.archiver.is_some() {
            Ok(())
        } else {
            Err(missing_archiver_error())
        }
    }

    pub fn compile_llvm_ir_to_object(
        &self,
        input_ir: &Path,
        output_object: &Path,
    ) -> Result<(), ToolchainError> {
        self.run_clang([
            "-c".to_owned(),
            "-x".to_owned(),
            "ir".to_owned(),
            input_ir.display().to_string(),
            "-o".to_owned(),
            output_object.display().to_string(),
        ])
    }

    pub fn link_object_to_executable(
        &self,
        input_object: &Path,
        output_executable: &Path,
    ) -> Result<(), ToolchainError> {
        self.run_clang([
            input_object.display().to_string(),
            "-o".to_owned(),
            output_executable.display().to_string(),
        ])
    }

    pub fn archive_object_to_static_library(
        &self,
        input_object: &Path,
        output_static_library: &Path,
    ) -> Result<(), ToolchainError> {
        let Some(archiver) = &self.archiver else {
            return Err(missing_archiver_error());
        };

        match archiver.flavor {
            ArchiverFlavor::Ar => self.run_program(
                &archiver.program,
                [
                    "rcs".to_owned(),
                    output_static_library.display().to_string(),
                    input_object.display().to_string(),
                ],
            ),
            ArchiverFlavor::Lib => self.run_program(
                &archiver.program,
                [
                    "/NOLOGO".to_owned(),
                    format!("/OUT:{}", output_static_library.display()),
                    input_object.display().to_string(),
                ],
            ),
        }
    }

    fn run_clang<I, S>(&self, args: I) -> Result<(), ToolchainError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.run_program(&self.clang, args)
    }

    fn run_program<I, S>(
        &self,
        invocation: &ProgramInvocation,
        args: I,
    ) -> Result<(), ToolchainError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut command = Command::new(&invocation.program);
        command.args(&invocation.args_prefix);
        command.args(args.into_iter().map(Into::into));

        let output = command
            .output()
            .map_err(|error| ToolchainError::InvocationFailed {
                program: invocation.display_name(),
                status: None,
                stderr: format!("failed to spawn toolchain process: {error}"),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            Err(ToolchainError::InvocationFailed {
                program: invocation.display_name(),
                status: output.status.code(),
                stderr,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolchainError {
    NotFound {
        tool: &'static str,
        hint: String,
    },
    InvocationFailed {
        program: String,
        status: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tool, hint } => {
                write!(f, "required toolchain `{tool}` was not found; {hint}")
            }
            Self::InvocationFailed {
                program,
                status,
                stderr,
            } => {
                write!(f, "toolchain `{program}` failed")?;
                if let Some(status) = status {
                    write!(f, " with exit code {status}")?;
                }
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
        }
    }
}

pub fn discover_toolchain(
    options: &ToolchainOptions,
) -> Result<DiscoveredToolchain, ToolchainError> {
    let clang = discover_clang(options)?;
    let archiver = discover_archiver(options);

    Ok(DiscoveredToolchain { clang, archiver })
}

fn discover_clang(options: &ToolchainOptions) -> Result<ProgramInvocation, ToolchainError> {
    if let Some(clang) = &options.clang {
        return Ok(clang.clone());
    }

    if let Ok(override_path) = env::var("QLANG_CLANG") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return Ok(ProgramInvocation::new(trimmed));
        }
    }

    find_program_on_path(&clang_candidates())
        .map(ProgramInvocation::new)
        .ok_or_else(|| ToolchainError::NotFound {
            tool: "clang",
            hint: "install clang on PATH or set `QLANG_CLANG` to an explicit compiler path"
                .to_owned(),
        })
}

fn discover_archiver(options: &ToolchainOptions) -> Option<ArchiverInvocation> {
    if let Some(archiver) = &options.archiver {
        return Some(archiver.clone());
    }

    if let Ok(override_path) = env::var("QLANG_AR") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            let program = ProgramInvocation::new(trimmed);
            let style_override = env::var("QLANG_AR_STYLE").ok();
            return Some(ArchiverInvocation {
                flavor: archiver_flavor_from_override(&program.program, style_override.as_deref()),
                program,
            });
        }
    }

    find_named_program_on_path(&archiver_candidates()).map(|(program, flavor)| ArchiverInvocation {
        program: ProgramInvocation::new(program),
        flavor,
    })
}

fn find_program_on_path(candidates: &[&str]) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        for candidate in candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn find_named_program_on_path(
    candidates: &[(&str, ArchiverFlavor)],
) -> Option<(PathBuf, ArchiverFlavor)> {
    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        for (candidate, flavor) in candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some((path, *flavor));
            }
        }
    }
    None
}

fn clang_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["clang.exe", "clang.cmd", "clang.bat", "clang"]
    } else {
        vec!["clang"]
    }
}

fn archiver_candidates() -> Vec<(&'static str, ArchiverFlavor)> {
    if cfg!(windows) {
        vec![
            ("llvm-ar.exe", ArchiverFlavor::Ar),
            ("llvm-ar.cmd", ArchiverFlavor::Ar),
            ("llvm-ar.bat", ArchiverFlavor::Ar),
            ("llvm-ar", ArchiverFlavor::Ar),
            ("llvm-lib.exe", ArchiverFlavor::Lib),
            ("llvm-lib.cmd", ArchiverFlavor::Lib),
            ("llvm-lib.bat", ArchiverFlavor::Lib),
            ("llvm-lib", ArchiverFlavor::Lib),
            ("lib.exe", ArchiverFlavor::Lib),
            ("lib.cmd", ArchiverFlavor::Lib),
            ("lib.bat", ArchiverFlavor::Lib),
            ("lib", ArchiverFlavor::Lib),
        ]
    } else {
        vec![("llvm-ar", ArchiverFlavor::Ar), ("ar", ArchiverFlavor::Ar)]
    }
}

fn infer_archiver_flavor(program: &Path) -> ArchiverFlavor {
    let name = program
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();

    if name.contains("lib") && !name.contains("ar") {
        ArchiverFlavor::Lib
    } else {
        ArchiverFlavor::Ar
    }
}

fn archiver_flavor_from_override(program: &Path, style_override: Option<&str>) -> ArchiverFlavor {
    match style_override.map(|style| style.trim().to_ascii_lowercase()) {
        Some(style) if style == "lib" => ArchiverFlavor::Lib,
        Some(style) if style == "ar" => ArchiverFlavor::Ar,
        _ => infer_archiver_flavor(program),
    }
}

fn missing_archiver_error() -> ToolchainError {
    ToolchainError::NotFound {
        tool: "archiver",
        hint:
            "install `llvm-ar`, `ar`, or a `lib.exe`-compatible archiver on PATH, or set `QLANG_AR` to an explicit archive tool"
                .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ArchiverFlavor, archiver_flavor_from_override};

    #[test]
    fn archiver_override_style_can_force_lib_flavor_for_wrappers() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/tmp/mock-archiver.cmd"), Some("lib")),
            ArchiverFlavor::Lib
        );
    }

    #[test]
    fn archiver_override_style_can_force_ar_flavor_for_wrappers() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/tmp/mock-archiver.cmd"), Some("ar")),
            ArchiverFlavor::Ar
        );
    }

    #[test]
    fn archiver_override_falls_back_to_program_name_inference() {
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/LLVM/bin/llvm-lib.exe"), None),
            ArchiverFlavor::Lib
        );
        assert_eq!(
            archiver_flavor_from_override(Path::new("C:/LLVM/bin/llvm-ar.exe"), None),
            ArchiverFlavor::Ar
        );
    }
}
