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
    pub fn clang(&self) -> &ProgramInvocation {
        &self.clang
    }

    pub fn archiver(&self) -> Option<&ArchiverInvocation> {
        self.archiver.as_ref()
    }

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

    pub fn compile_llvm_ir_to_assembly(
        &self,
        input_ir: &Path,
        output_assembly: &Path,
    ) -> Result<(), ToolchainError> {
        self.run_clang([
            "-S".to_owned(),
            "-x".to_owned(),
            "ir".to_owned(),
            input_ir.display().to_string(),
            "-o".to_owned(),
            output_assembly.display().to_string(),
        ])
    }

    pub fn link_object_to_executable(
        &self,
        input_object: &Path,
        output_executable: &Path,
    ) -> Result<(), ToolchainError> {
        self.link_object_to_executable_with_inputs(input_object, output_executable, &[])
    }

    pub fn link_object_to_executable_with_inputs(
        &self,
        input_object: &Path,
        output_executable: &Path,
        additional_inputs: &[PathBuf],
    ) -> Result<(), ToolchainError> {
        let mut args = vec![input_object.display().to_string()];
        args.extend(
            additional_inputs
                .iter()
                .map(|path| path.display().to_string()),
        );
        args.push("-o".to_owned());
        args.push(output_executable.display().to_string());
        self.run_clang(args)
    }

    pub fn link_object_to_dynamic_library(
        &self,
        input_object: &Path,
        output_dynamic_library: &Path,
        exported_symbols: &[String],
    ) -> Result<(), ToolchainError> {
        self.link_object_to_dynamic_library_with_inputs(
            input_object,
            output_dynamic_library,
            exported_symbols,
            &[],
        )
    }

    pub fn link_object_to_dynamic_library_with_inputs(
        &self,
        input_object: &Path,
        output_dynamic_library: &Path,
        exported_symbols: &[String],
        additional_inputs: &[PathBuf],
    ) -> Result<(), ToolchainError> {
        let mut args = if cfg!(target_os = "macos") {
            vec!["-dynamiclib".to_owned()]
        } else {
            vec!["-shared".to_owned()]
        };
        args.push(input_object.display().to_string());
        args.extend(
            additional_inputs
                .iter()
                .map(|path| path.display().to_string()),
        );
        args.push("-o".to_owned());
        args.push(output_dynamic_library.display().to_string());

        if cfg!(windows) {
            for symbol in exported_symbols {
                args.push("-Xlinker".to_owned());
                args.push(format!("/EXPORT:{symbol}"));
            }
        }

        self.run_clang(args)
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
        .or_else(discover_clang_in_common_windows_locations)
        .map(ProgramInvocation::new)
        .ok_or_else(|| ToolchainError::NotFound {
            tool: "clang",
            hint: missing_clang_hint(),
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

    find_named_program_on_path(&archiver_candidates())
        .map(|(program, flavor)| ArchiverInvocation {
            program: ProgramInvocation::new(program),
            flavor,
        })
        .or_else(discover_archiver_in_common_windows_locations)
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

fn find_program_in_directories(candidates: &[&str], directories: &[PathBuf]) -> Option<PathBuf> {
    for directory in directories {
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

fn find_named_program_in_directories(
    candidates: &[(&str, ArchiverFlavor)],
    directories: &[PathBuf],
) -> Option<(PathBuf, ArchiverFlavor)> {
    for directory in directories {
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WindowsToolchainRoots {
    scoop: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    local_appdata: Option<PathBuf>,
    program_files: Option<PathBuf>,
    program_files_x86: Option<PathBuf>,
}

impl WindowsToolchainRoots {
    fn from_env() -> Self {
        Self {
            scoop: env::var_os("SCOOP").map(PathBuf::from),
            user_profile: env::var_os("USERPROFILE").map(PathBuf::from),
            local_appdata: env::var_os("LOCALAPPDATA").map(PathBuf::from),
            program_files: env::var_os("ProgramFiles").map(PathBuf::from),
            program_files_x86: env::var_os("ProgramFiles(x86)").map(PathBuf::from),
        }
    }
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn windows_llvm_bin_dirs_from_roots(roots: &WindowsToolchainRoots) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(scoop) = &roots.scoop {
        push_unique_path(
            &mut directories,
            scoop.join("apps").join("llvm").join("current").join("bin"),
        );
    }
    if let Some(user_profile) = &roots.user_profile {
        push_unique_path(
            &mut directories,
            user_profile
                .join("scoop")
                .join("apps")
                .join("llvm")
                .join("current")
                .join("bin"),
        );
    }
    if let Some(local_appdata) = &roots.local_appdata {
        push_unique_path(
            &mut directories,
            local_appdata.join("Programs").join("LLVM").join("bin"),
        );
    }
    if let Some(program_files) = &roots.program_files {
        push_unique_path(&mut directories, program_files.join("LLVM").join("bin"));
    }
    if let Some(program_files_x86) = &roots.program_files_x86 {
        push_unique_path(&mut directories, program_files_x86.join("LLVM").join("bin"));
    }
    directories
}

fn common_windows_llvm_bin_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        windows_llvm_bin_dirs_from_roots(&WindowsToolchainRoots::from_env())
    } else {
        Vec::new()
    }
}

fn discover_clang_in_common_windows_locations() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let directories = common_windows_llvm_bin_dirs();
    find_program_in_directories(&clang_candidates(), &directories)
}

fn discover_archiver_in_common_windows_locations() -> Option<ArchiverInvocation> {
    if !cfg!(windows) {
        return None;
    }
    let directories = common_windows_llvm_bin_dirs();
    find_named_program_in_directories(&archiver_candidates(), &directories).map(
        |(program, flavor)| ArchiverInvocation {
            program: ProgramInvocation::new(program),
            flavor,
        },
    )
}

fn suggested_program_paths(
    candidates: &[&str],
    directories: &[PathBuf],
    limit: usize,
) -> Vec<PathBuf> {
    let mut suggestions = Vec::new();
    for directory in directories {
        for candidate in candidates {
            push_unique_path(&mut suggestions, directory.join(candidate));
            if suggestions.len() >= limit {
                return suggestions;
            }
        }
    }
    suggestions
}

fn suggested_named_program_paths(
    candidates: &[(&str, ArchiverFlavor)],
    directories: &[PathBuf],
    limit: usize,
) -> Vec<PathBuf> {
    let mut suggestions = Vec::new();
    for directory in directories {
        for (candidate, _) in candidates {
            push_unique_path(&mut suggestions, directory.join(candidate));
            if suggestions.len() >= limit {
                return suggestions;
            }
        }
    }
    suggestions
}

fn render_suggested_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| format!("`{}`", path.display()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn missing_clang_hint() -> String {
    let mut hint =
        "install clang on PATH or set `QLANG_CLANG` to an explicit compiler path".to_owned();
    if cfg!(windows) {
        let suggestions =
            suggested_program_paths(&clang_candidates(), &common_windows_llvm_bin_dirs(), 3);
        if !suggestions.is_empty() {
            hint.push_str("; also checked common Windows LLVM locations such as ");
            hint.push_str(&render_suggested_paths(&suggestions));
        }
        hint.push_str("; Scoop users can install LLVM with `scoop install llvm`");
    }
    hint
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
    let mut hint =
        "install `llvm-ar`, `ar`, or a `lib.exe`-compatible archiver on PATH, or set `QLANG_AR` to an explicit archive tool"
            .to_owned();
    if cfg!(windows) {
        let suggestions = suggested_named_program_paths(
            &archiver_candidates(),
            &common_windows_llvm_bin_dirs(),
            4,
        );
        if !suggestions.is_empty() {
            hint.push_str("; also checked common Windows LLVM locations such as ");
            hint.push_str(&render_suggested_paths(&suggestions));
        }
        hint.push_str(
            "; use `QLANG_AR_STYLE=lib|ar` when a wrapper name does not imply the archive flavor",
        );
    }
    ToolchainError::NotFound {
        tool: "archiver",
        hint,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ArchiverFlavor, WindowsToolchainRoots, archiver_flavor_from_override,
        find_program_in_directories, missing_archiver_error, missing_clang_hint,
        windows_llvm_bin_dirs_from_roots,
    };

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary toolchain test directory");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

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

    #[test]
    fn find_program_in_directories_returns_matching_candidate() {
        let temp = TempDir::new("ql-driver-toolchain-find");
        let bin = temp.path.join("bin");
        fs::create_dir_all(&bin).expect("create fake bin dir");
        let clang = bin.join("clang.cmd");
        fs::write(&clang, "@echo off\r\n").expect("write fake clang wrapper");

        let found = find_program_in_directories(&["clang.exe", "clang.cmd"], &[bin]);

        assert_eq!(found, Some(clang));
    }

    #[test]
    fn windows_llvm_bin_dirs_cover_common_install_roots_without_duplicates() {
        let roots = WindowsToolchainRoots {
            scoop: Some(PathBuf::from("C:/Scoop")),
            user_profile: Some(PathBuf::from("C:/Users/alice")),
            local_appdata: Some(PathBuf::from("C:/Users/alice/AppData/Local")),
            program_files: Some(PathBuf::from("C:/Program Files")),
            program_files_x86: Some(PathBuf::from("C:/Program Files (x86)")),
        };

        let dirs = windows_llvm_bin_dirs_from_roots(&roots);

        assert!(dirs.contains(&PathBuf::from("C:/Scoop/apps/llvm/current/bin")));
        assert!(dirs.contains(&PathBuf::from("C:/Users/alice/scoop/apps/llvm/current/bin")));
        assert!(dirs.contains(&PathBuf::from(
            "C:/Users/alice/AppData/Local/Programs/LLVM/bin"
        )));
        assert!(dirs.contains(&PathBuf::from("C:/Program Files/LLVM/bin")));
        assert!(dirs.contains(&PathBuf::from("C:/Program Files (x86)/LLVM/bin")));
        assert_eq!(dirs.len(), 5);
    }

    #[cfg(windows)]
    #[test]
    fn missing_clang_hint_mentions_windows_candidates() {
        let hint = missing_clang_hint();

        assert!(hint.contains("QLANG_CLANG"));
        assert!(hint.contains("Scoop users can install LLVM with `scoop install llvm`"));
        assert!(hint.contains("clang.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn missing_archiver_hint_mentions_windows_candidates() {
        let error = missing_archiver_error();
        let rendered = error.to_string();

        assert!(rendered.contains("QLANG_AR"));
        assert!(rendered.contains("QLANG_AR_STYLE=lib|ar"));
        assert!(rendered.contains("llvm-lib.exe") || rendered.contains("llvm-ar.exe"));
    }
}
