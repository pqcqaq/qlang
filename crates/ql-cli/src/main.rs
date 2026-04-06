use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use ql_analysis::{
    PackageAnalysisError, analyze_package, analyze_source as analyze_semantics,
    parse_errors_to_diagnostics,
};
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_driver::{
    BuildCHeaderOptions, BuildEmit, BuildError, BuildOptions, BuildProfile, CHeaderError,
    CHeaderOptions, CHeaderSurface, build_file, emit_c_header,
};
use ql_fmt::format_source;
use ql_project::{
    InterfaceArtifactStatus, collect_package_sources, default_interface_path,
    interface_artifact_status, load_project_manifest, package_name, package_source_root,
    render_module_interface, render_project_graph_resolved,
};
use ql_runtime::{collect_runtime_hook_signatures, collect_runtime_hooks};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run() -> Result<(), u8> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Err(1);
    };

    match command.as_str() {
        "check" => {
            let remaining = args.collect::<Vec<_>>();
            let mut path = None;
            let mut sync_interfaces = false;
            for arg in remaining {
                match arg.as_str() {
                    "--sync-interfaces" => {
                        sync_interfaces = true;
                    }
                    other if other.starts_with('-') => {
                        eprintln!("error: unknown `ql check` option `{other}`");
                        return Err(1);
                    }
                    other => {
                        if path.is_some() {
                            eprintln!("error: unknown `ql check` argument `{other}`");
                            return Err(1);
                        }
                        path = Some(other.to_owned());
                    }
                }
            }
            let Some(path) = path else {
                eprintln!("error: `ql check` expects a file or directory path");
                return Err(1);
            };
            check_path(Path::new(&path), sync_interfaces)
        }
        "fmt" => {
            let mut write = false;
            let mut path = None;
            for arg in args {
                if arg == "--write" {
                    write = true;
                } else {
                    path = Some(arg);
                }
            }

            let Some(path) = path else {
                eprintln!("error: `ql fmt` expects a file path");
                return Err(1);
            };

            format_path(Path::new(&path), write)
        }
        "mir" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql mir` expects a file path");
                return Err(1);
            };

            render_mir_path(Path::new(&path))
        }
        "ownership" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql ownership` expects a file path");
                return Err(1);
            };

            render_ownership_path(Path::new(&path))
        }
        "runtime" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql runtime` expects a file path");
                return Err(1);
            };

            render_runtime_requirements_path(Path::new(&path))
        }
        "build" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql build` expects a file path");
                return Err(1);
            };

            let mut options = BuildOptions::default();
            let mut emit_interface = false;
            let remaining = args.collect::<Vec<_>>();
            let mut index = 0;

            while index < remaining.len() {
                match remaining[index].as_str() {
                    "--emit" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --emit` expects a value");
                            return Err(1);
                        };
                        match value.as_str() {
                            "llvm-ir" => options.emit = BuildEmit::LlvmIr,
                            "obj" => options.emit = BuildEmit::Object,
                            "exe" => options.emit = BuildEmit::Executable,
                            "dylib" => options.emit = BuildEmit::DynamicLibrary,
                            "staticlib" => options.emit = BuildEmit::StaticLibrary,
                            other => {
                                eprintln!("error: unsupported build emit target `{other}`");
                                return Err(1);
                            }
                        }
                    }
                    "--release" => {
                        options.profile = BuildProfile::Release;
                    }
                    "-o" | "--output" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --output` expects a file path");
                            return Err(1);
                        };
                        options.output = Some(PathBuf::from(value));
                    }
                    "--header" => {
                        options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                    }
                    "--emit-interface" => {
                        emit_interface = true;
                    }
                    "--header-surface" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!(
                                "error: `ql build --header-surface` expects `exports`, `imports`, or `both`"
                            );
                            return Err(1);
                        };
                        let Some(surface) = CHeaderSurface::parse(value) else {
                            eprintln!("error: unsupported `ql build` header surface `{value}`");
                            return Err(1);
                        };
                        let header = options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                        header.surface = surface;
                    }
                    "--header-output" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --header-output` expects a file path");
                            return Err(1);
                        };
                        let header = options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                        header.output = Some(PathBuf::from(value));
                    }
                    other => {
                        eprintln!("error: unknown `ql build` option `{other}`");
                        return Err(1);
                    }
                }

                index += 1;
            }

            build_path(Path::new(&path), &options, emit_interface)
        }
        "project" => {
            let Some(subcommand) = args.next() else {
                eprintln!("error: `ql project` expects a subcommand");
                return Err(1);
            };

            match subcommand.as_str() {
                "graph" => {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .or_else(|| env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    if let Some(extra) = args.next() {
                        eprintln!("error: unknown `ql project graph` argument `{extra}`");
                        return Err(1);
                    }
                    project_graph_path(&path)
                }
                "emit-interface" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut output = None;
                    let mut changed_only = false;
                    let mut check_only = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "-o" | "--output" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project emit-interface --output` expects a file path"
                                    );
                                    return Err(1);
                                };
                                output = Some(PathBuf::from(value));
                            }
                            "--changed-only" => {
                                changed_only = true;
                            }
                            "--check" => {
                                check_only = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!(
                                    "error: unknown `ql project emit-interface` option `{other}`"
                                );
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project emit-interface` argument `{other}`"
                                    );
                                    return Err(1);
                                }
                                path = Some(PathBuf::from(other));
                            }
                        }

                        index += 1;
                    }

                    let path = path
                        .or_else(|| env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    project_emit_interface_path(&path, output.as_deref(), changed_only, check_only)
                }
                other => {
                    eprintln!("error: unknown `ql project` subcommand `{other}`");
                    print_usage();
                    Err(1)
                }
            }
        }
        "ffi" => {
            let Some(subcommand) = args.next() else {
                eprintln!("error: `ql ffi` expects a subcommand");
                return Err(1);
            };

            match subcommand.as_str() {
                "header" => {
                    let Some(path) = args.next() else {
                        eprintln!("error: `ql ffi header` expects a file path");
                        return Err(1);
                    };

                    let mut options = CHeaderOptions::default();
                    let remaining = args.collect::<Vec<_>>();
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "-o" | "--output" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql ffi header --output` expects a file path"
                                    );
                                    return Err(1);
                                };
                                options.output = Some(PathBuf::from(value));
                            }
                            "--surface" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql ffi header --surface` expects `exports`, `imports`, or `both`"
                                    );
                                    return Err(1);
                                };
                                let Some(surface) = CHeaderSurface::parse(value) else {
                                    eprintln!(
                                        "error: unsupported `ql ffi header` surface `{value}`"
                                    );
                                    return Err(1);
                                };
                                options.surface = surface;
                            }
                            other => {
                                eprintln!("error: unknown `ql ffi header` option `{other}`");
                                return Err(1);
                            }
                        }

                        index += 1;
                    }

                    emit_c_header_path(Path::new(&path), &options)
                }
                other => {
                    eprintln!("error: unknown `ql ffi` subcommand `{other}`");
                    print_usage();
                    Err(1)
                }
            }
        }
        _ => {
            eprintln!("error: unknown command `{command}`");
            print_usage();
            Err(1)
        }
    }
}

fn check_path(path: &Path, sync_interfaces: bool) -> Result<(), u8> {
    let use_package_check = should_use_package_check(path)
        || (is_ql_source_file(path) && load_project_manifest(path).is_ok());
    if use_package_check {
        if let Ok(manifest) = load_project_manifest(path) {
            if manifest.package.is_none() && manifest.workspace.is_some() {
                return check_workspace_manifest(&manifest, sync_interfaces);
            }
            if !sync_interfaces {
                ensure_reference_interfaces_current(&manifest)?;
            }
        }
        if sync_interfaces {
            for interface_path in sync_reference_interfaces(path, &mut BTreeSet::new())? {
                println!("wrote interface: {}", interface_path.display());
            }
        }
        match analyze_package(path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    return Err(1);
                }
                for module in package.modules() {
                    println!("ok: {}", module.path().display());
                }
                for dependency in package.dependencies() {
                    println!(
                        "loaded interface: {}",
                        dependency.interface_path().display()
                    );
                }
                return Ok(());
            }
            Err(PackageAnalysisError::Project(ql_project::ProjectError::ManifestNotFound {
                ..
            })) => {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
                {
                    eprintln!(
                        "error: could not find `qlang.toml` starting from `{}`",
                        path.display()
                    );
                    return Err(1);
                }
            }
            Err(error) => {
                print_package_analysis_error(&error);
                return Err(1);
            }
        }
    }

    let files = collect_ql_files(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;

    if files.is_empty() {
        eprintln!("error: no `.ql` files found under `{}`", path.display());
        return Err(1);
    }

    let mut has_errors = false;

    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            eprintln!("error: failed to read `{}`: {error}", file.display());
            1
        })?;

        match analyze_source(&source) {
            Ok(()) => println!("ok: {}", file.display()),
            Err(diagnostics) => {
                has_errors = true;
                print_diagnostics(&file, &source, &diagnostics);
            }
        }
    }

    if has_errors { Err(1) } else { Ok(()) }
}

fn check_workspace_manifest(
    manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
) -> Result<(), u8> {
    let Some(workspace) = &manifest.workspace else {
        return Ok(());
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let mut sync_visited = BTreeSet::new();
    let mut synced_interfaces = BTreeSet::new();

    for member in &workspace.members {
        let member_path = manifest_dir.join(member);
        let member_manifest = load_project_manifest(&member_path).map_err(|error| {
            eprintln!("error: {error}");
            1
        })?;

        if !sync_interfaces {
            ensure_reference_interfaces_current(&member_manifest)?;
        }

        if sync_interfaces {
            for interface_path in sync_reference_interfaces(&member_path, &mut sync_visited)? {
                let display_path =
                    fs::canonicalize(&interface_path).unwrap_or_else(|_| interface_path.clone());
                if synced_interfaces.insert(display_path.clone()) {
                    println!("wrote interface: {}", display_path.display());
                }
            }
        }

        match analyze_package(&member_path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    return Err(1);
                }
                for module in package.modules() {
                    println!("ok: {}", module.path().display());
                }
                for dependency in package.dependencies() {
                    println!(
                        "loaded interface: {}",
                        dependency.interface_path().display()
                    );
                }
            }
            Err(error) => {
                print_package_analysis_error(&error);
                return Err(1);
            }
        }
    }

    Ok(())
}

fn format_path(path: &Path, write: bool) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match format_source(&source) {
        Ok(formatted) => {
            if write {
                fs::write(path, formatted).map_err(|error| {
                    eprintln!("error: failed to write `{}`: {error}", path.display());
                    1
                })?;
            } else {
                print!("{formatted}");
            }
            Ok(())
        }
        Err(errors) => {
            print_diagnostics(path, &source, &parse_errors_to_diagnostics(errors));
            Err(1)
        }
    }
}

fn render_mir_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", analysis.render_mir());
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn render_ownership_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", analysis.render_borrowck());
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn render_runtime_requirements_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", render_runtime_requirements(&analysis));
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn build_path(path: &Path, options: &BuildOptions, emit_interface: bool) -> Result<(), u8> {
    match build_file(path, options) {
        Ok(artifact) => {
            println!(
                "wrote {}: {}",
                artifact.emit.as_str(),
                artifact.path.display()
            );
            if let Some(header) = artifact.c_header {
                println!("wrote c-header: {}", header.path.display());
            }
            if emit_interface {
                report_emit_interface_result(emit_package_interface_path(
                    path,
                    None,
                    "`ql build --emit-interface`",
                    false,
                )?);
            }
            Ok(())
        }
        Err(BuildError::InvalidInput(message)) => {
            eprintln!("error: {message}");
            Err(1)
        }
        Err(BuildError::Io { path, error }) => {
            eprintln!("error: failed to access `{}`: {error}", path.display());
            Err(1)
        }
        Err(BuildError::Toolchain {
            error,
            preserved_artifacts,
        }) => {
            eprintln!("error: {error}");
            for path in preserved_artifacts {
                eprintln!(
                    "note: preserved intermediate artifact at `{}`",
                    path.display()
                );
            }
            Err(1)
        }
        Err(BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        }) => {
            print_diagnostics(&path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn emit_c_header_path(path: &Path, options: &CHeaderOptions) -> Result<(), u8> {
    match emit_c_header(path, options) {
        Ok(artifact) => {
            println!("wrote c-header: {}", artifact.path.display());
            Ok(())
        }
        Err(CHeaderError::InvalidInput(message)) => {
            eprintln!("error: {message}");
            Err(1)
        }
        Err(CHeaderError::Io { path, error }) => {
            eprintln!("error: failed to access `{}`: {error}", path.display());
            Err(1)
        }
        Err(CHeaderError::Diagnostics {
            path,
            source,
            diagnostics,
        }) => {
            print_diagnostics(&path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn project_graph_path(path: &Path) -> Result<(), u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    let rendered = render_project_graph_resolved(&manifest).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    print!("{rendered}");
    Ok(())
}

fn project_emit_interface_path(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> Result<(), u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;

    if check_only && output.is_some() {
        eprintln!("error: `ql project emit-interface --check` does not support `--output`");
        return Err(1);
    }

    if manifest.package.is_some() {
        if check_only {
            return report_package_interface_check(check_package_interface_artifact(
                &manifest,
                "`ql project emit-interface --check`",
            )?);
        }
        report_emit_interface_result(emit_package_interface_path(
            path,
            output,
            "`ql project emit-interface`",
            changed_only,
        )?);
        return Ok(());
    }

    if output.is_some() {
        eprintln!("error: `ql project emit-interface --output` only supports package manifests");
        return Err(1);
    }

    let Some(workspace) = &manifest.workspace else {
        eprintln!("error: `ql project emit-interface` requires `[package]` or `[workspace]`");
        return Err(1);
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let mut invalid_count = 0usize;
    for member in &workspace.members {
        let member_manifest =
            load_project_manifest(&manifest_dir.join(member)).map_err(|error| {
                eprintln!("error: {error}");
                1
            })?;
        if check_only {
            let result = check_package_interface_artifact(
                &member_manifest,
                "`ql project emit-interface --check`",
            )?;
            if report_package_interface_check(result).is_err() {
                invalid_count += 1;
            }
        } else {
            report_emit_interface_result(emit_package_interface_path(
                &manifest_dir.join(member),
                None,
                "`ql project emit-interface`",
                changed_only,
            )?);
        }
    }

    if check_only && invalid_count > 0 {
        eprintln!("error: interface check found {invalid_count} invalid artifact(s)");
        return Err(1);
    }

    Ok(())
}

enum EmitPackageInterfaceResult {
    Wrote(PathBuf),
    UpToDate(PathBuf),
}

enum CheckPackageInterfaceResult {
    Ok(PathBuf),
    Invalid {
        path: PathBuf,
        status: InterfaceArtifactStatus,
        manifest_path: PathBuf,
    },
}

fn emit_package_interface_path(
    path: &Path,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<EmitPackageInterfaceResult, u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        eprintln!("error: {command_label} {error}");
        1
    })?;
    let output_path = output.map(Path::to_path_buf).unwrap_or_else(|| {
        default_interface_path(&manifest).expect("package emit should have a default qi path")
    });
    if changed_only
        && interface_artifact_status(&manifest, &output_path) == InterfaceArtifactStatus::Valid
    {
        return Ok(EmitPackageInterfaceResult::UpToDate(output_path));
    }

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let files = collect_package_sources(&manifest).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;

    let mut rendered_modules = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            eprintln!("error: failed to read `{}`: {error}", file.display());
            1
        })?;
        let analysis = match analyze_semantics(&source) {
            Ok(analysis) => analysis,
            Err(diagnostics) => {
                print_diagnostics(&file, &source, &diagnostics);
                return Err(1);
            }
        };
        if analysis.has_errors() {
            print_diagnostics(&file, &source, analysis.diagnostics());
            return Err(1);
        }
        if let Some(rendered) = render_module_interface(analysis.ast()) {
            let relative = file.strip_prefix(manifest_dir).unwrap_or(&file);
            rendered_modules.push((normalize_path(relative), rendered));
        }
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!(
                "error: failed to create interface output directory `{}`: {error}",
                parent.display()
            );
            1
        })?;
    }

    let rendered = render_interface_artifact(package_name, &rendered_modules);
    fs::write(&output_path, rendered).map_err(|error| {
        eprintln!(
            "error: failed to write interface `{}`: {error}",
            output_path.display()
        );
        1
    })?;
    Ok(EmitPackageInterfaceResult::Wrote(output_path))
}

fn report_emit_interface_result(result: EmitPackageInterfaceResult) {
    match result {
        EmitPackageInterfaceResult::Wrote(path) => {
            println!("wrote interface: {}", path.display());
        }
        EmitPackageInterfaceResult::UpToDate(path) => {
            println!("up-to-date interface: {}", path.display());
        }
    }
}

fn check_package_interface_artifact(
    manifest: &ql_project::ProjectManifest,
    command_label: &str,
) -> Result<CheckPackageInterfaceResult, u8> {
    let output_path = default_interface_path(manifest).map_err(|error| {
        eprintln!("error: {command_label} {error}");
        1
    })?;
    let status = interface_artifact_status(manifest, &output_path);
    if status != InterfaceArtifactStatus::Valid {
        return Ok(CheckPackageInterfaceResult::Invalid {
            path: output_path,
            status,
            manifest_path: manifest.manifest_path.clone(),
        });
    }
    Ok(CheckPackageInterfaceResult::Ok(output_path))
}

fn report_package_interface_check(result: CheckPackageInterfaceResult) -> Result<(), u8> {
    match result {
        CheckPackageInterfaceResult::Ok(path) => {
            println!("ok interface: {}", path.display());
            Ok(())
        }
        CheckPackageInterfaceResult::Invalid {
            path,
            status,
            manifest_path,
        } => {
            eprintln!(
                "error: interface artifact `{}` is {}",
                path.display(),
                status.label()
            );
            eprintln!(
                "hint: rerun `ql project emit-interface {}` to regenerate it",
                manifest_path.display()
            );
            Err(1)
        }
    }
}

fn sync_reference_interfaces(
    path: &Path,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<Vec<PathBuf>, u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    let manifest_path = manifest.manifest_path.clone();
    if !visited.insert(manifest_path) {
        return Ok(Vec::new());
    }

    let mut written = Vec::new();
    for dependency_manifest in ql_project::load_reference_manifests(&manifest).map_err(|error| {
        eprintln!("error: {error}");
        1
    })? {
        written.extend(sync_reference_interfaces(
            &dependency_manifest.manifest_path,
            visited,
        )?);
        if should_sync_interface_artifact(&dependency_manifest)? {
            let result = emit_package_interface_path(
                &dependency_manifest.manifest_path,
                None,
                "`ql check --sync-interfaces`",
                false,
            )?;
            if let EmitPackageInterfaceResult::Wrote(path) = result {
                written.push(path);
            }
        }
    }
    Ok(written)
}

fn ensure_reference_interfaces_current(manifest: &ql_project::ProjectManifest) -> Result<(), u8> {
    ensure_reference_interfaces_current_recursive(manifest, &mut BTreeSet::new())
}

fn ensure_reference_interfaces_current_recursive(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), u8> {
    let manifest_path = manifest.manifest_path.clone();
    if !visited.insert(manifest_path.clone()) {
        return Ok(());
    }

    for dependency_manifest in ql_project::load_reference_manifests(manifest).map_err(|error| {
        eprintln!("error: {error}");
        1
    })? {
        let dependency_package = package_name(&dependency_manifest).map_err(|error| {
            eprintln!("error: {error}");
            1
        })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            eprintln!("error: {error}");
            1
        })?;
        if interface_artifact_status(&dependency_manifest, &interface_path)
            == InterfaceArtifactStatus::Stale
        {
            eprintln!(
                "error: referenced package `{dependency_package}` has stale interface artifact `{}`",
                interface_path.display()
            );
            eprintln!(
                "hint: rerun `ql check --sync-interfaces {}` or regenerate `{}` with `ql project emit-interface {}`",
                manifest_path.display(),
                dependency_package,
                dependency_manifest.manifest_path.display()
            );
            return Err(1);
        }
        ensure_reference_interfaces_current_recursive(&dependency_manifest, visited)?;
    }

    Ok(())
}

fn should_sync_interface_artifact(manifest: &ql_project::ProjectManifest) -> Result<bool, u8> {
    let interface_path = default_interface_path(manifest).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    Ok(interface_artifact_status(manifest, &interface_path) != InterfaceArtifactStatus::Valid)
}

fn analyze_source(source: &str) -> Result<(), Vec<Diagnostic>> {
    let analysis = analyze_semantics(source)?;
    if analysis.has_errors() {
        Err(analysis.diagnostics().to_vec())
    } else {
        Ok(())
    }
}

fn render_runtime_requirements(analysis: &ql_analysis::Analysis) -> String {
    if analysis.runtime_requirements().is_empty() {
        return "runtime requirements: none\n".to_owned();
    }

    let mut rendered = String::new();
    for requirement in analysis.runtime_requirements() {
        rendered.push_str(&format!(
            "runtime requirement: {} @ {} ({})\n",
            requirement.capability.stable_name(),
            requirement.span,
            requirement.capability.description(),
        ));
    }
    let capabilities = analysis
        .runtime_requirements()
        .iter()
        .map(|requirement| requirement.capability)
        .collect::<Vec<_>>();
    for hook in collect_runtime_hooks(capabilities.iter().copied()) {
        rendered.push_str(&format!(
            "runtime hook: {} -> {} ({})\n",
            hook.stable_name(),
            hook.symbol_name(),
            hook.description(),
        ));
    }
    for signature in collect_runtime_hook_signatures(capabilities.iter().copied()) {
        rendered.push_str(&format!(
            "runtime hook abi: {} {}\n",
            signature.hook.stable_name(),
            signature.render_contract(),
        ));
    }
    rendered
}

fn collect_ql_files(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    collect_ql_files_recursive(path, path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ql_files_recursive(
    root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            if should_skip_directory(root, &entry_path) {
                continue;
            }
            collect_ql_files_recursive(root, &entry_path, files)?;
        } else if is_ql_file(&entry_path) && !should_skip_file(root, &entry_path) {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn is_ql_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("ql")
}

fn should_skip_directory(root: &Path, path: &Path) -> bool {
    if path == root {
        return false;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name.starts_with('.')
        || matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | "coverage" | "fixtures" | "ramdon_tests"
        )
        || is_negative_fixture_path(root, path)
}

fn should_skip_file(root: &Path, path: &Path) -> bool {
    if path == root {
        return false;
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
        || is_negative_fixture_path(root, path)
}

fn is_negative_fixture_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    let mut saw_fixtures = false;
    for component in relative.components().filter_map(component_name) {
        if component == "fixtures" {
            saw_fixtures = true;
            continue;
        }

        if saw_fixtures && component == "fail" {
            return true;
        }
    }

    false
}

fn component_name(component: Component<'_>) -> Option<&str> {
    match component {
        Component::Normal(segment) => segment.to_str(),
        _ => None,
    }
}

fn should_use_package_check(path: &Path) -> bool {
    path.is_dir()
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
}

fn is_ql_source_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ql"))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn render_interface_artifact(package_name: &str, modules: &[(String, String)]) -> String {
    let mut rendered = String::new();
    rendered.push_str("// qlang interface v1\n");
    rendered.push_str(&format!("// package: {package_name}\n"));
    if modules.is_empty() {
        rendered.push('\n');
        return rendered;
    }

    for (path, module) in modules {
        rendered.push('\n');
        rendered.push_str(&format!("// source: {path}\n"));
        rendered.push_str(module);
    }

    rendered
}

fn print_diagnostics(path: &Path, source: &str, diagnostics: &[Diagnostic]) {
    eprint!("{}", render_diagnostics(path, source, diagnostics));
}

fn print_package_analysis_error(error: &PackageAnalysisError) {
    match error {
        PackageAnalysisError::Project(error) => eprintln!("error: {error}"),
        PackageAnalysisError::Read { path, error } => {
            eprintln!("error: failed to read `{}`: {error}", path.display());
        }
        PackageAnalysisError::SourceDiagnostics {
            path,
            source,
            diagnostics,
        } => print_diagnostics(path, source, diagnostics),
        PackageAnalysisError::InterfaceNotFound { package_name, path } => {
            eprintln!(
                "error: referenced package `{package_name}` is missing interface artifact `{}`",
                path.display()
            );
        }
        PackageAnalysisError::InterfaceParse { path, message } => {
            eprintln!("error: invalid interface `{}`: {message}", path.display());
        }
    }
}

fn print_usage() {
    eprintln!("Qlang CLI");
    eprintln!("usage:");
    eprintln!("  ql check <file-or-dir> [--sync-interfaces]");
    eprintln!(
        "  ql build <file> [--emit llvm-ir|obj|exe|dylib|staticlib] [--release] [-o <output>] [--emit-interface] [--header] [--header-surface exports|imports|both] [--header-output <output>]"
    );
    eprintln!("  ql project graph [file-or-dir]");
    eprintln!("  ql project emit-interface [file-or-dir] [-o <output>] [--changed-only] [--check]");
    eprintln!("  ql ffi header <file> [--surface exports|imports|both] [-o <output>]");
    eprintln!("  ql fmt <file> [--write]");
    eprintln!("  ql mir <file>");
    eprintln!("  ql ownership <file>");
    eprintln!("  ql runtime <file>");
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ql_driver::{
        ArchiverFlavor, ArchiverInvocation, BuildEmit, BuildOptions, BuildProfile,
        ProgramInvocation, ToolchainOptions,
    };

    use super::{
        analyze_semantics, analyze_source, build_path, collect_ql_files, render_mir_path,
        render_ownership_path, render_runtime_requirements,
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

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

        let rendered = fs::read_to_string(output).expect("read emitted LLVM IR");
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
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

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

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

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

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

        assert!(build_path(&dir.path().join("ffi_export.ql"), &options).is_ok());

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

        assert!(build_path(&dir.path().join("math.ql"), &options).is_ok());

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
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
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
if ($isCompile) {
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
is_shared=0
while [ "$#" -gt 0 ]; do
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
if [ "$is_compile" -eq 1 ]; then
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
}
