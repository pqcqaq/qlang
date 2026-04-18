use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use ql_analysis::{
    PackageAnalysisError, analyze_package, analyze_source as analyze_semantics,
    parse_errors_to_diagnostics,
};
use ql_ast::{ItemKind, Module, Visibility};
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_driver::{
    BuildArtifact, BuildCHeaderOptions, BuildEmit, BuildError, BuildOptions, BuildProfile,
    CHeaderError, CHeaderOptions, CHeaderSurface, ToolchainError, build_file,
    build_file_with_link_inputs, build_source_with_link_inputs, default_output_path, emit_c_header,
};
use ql_fmt::format_source;
use ql_project::{
    BuildTarget, BuildTargetKind, InterfaceArtifactStaleReason, InterfaceArtifactStatus,
    ManifestBuildProfile, WorkspaceBuildTargets, collect_package_sources, default_interface_path,
    discover_package_build_targets, discover_workspace_build_targets,
    interface_artifact_stale_reasons, interface_artifact_status, interface_artifact_status_detail,
    load_interface_artifact, load_project_manifest, load_reference_manifests, package_name,
    package_source_root, project_lockfile_path, render_module_interface,
    render_project_graph_resolved, render_project_graph_resolved_json, render_project_lockfile,
};
use ql_runtime::{collect_runtime_hook_signatures, collect_runtime_hooks};
use ql_span::locate;
use serde_json::{Value as JsonValue, json};

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
            let mut json = false;
            for arg in remaining {
                match arg.as_str() {
                    "--sync-interfaces" => {
                        sync_interfaces = true;
                    }
                    "--json" => {
                        json = true;
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
            check_path(Path::new(&path), sync_interfaces, json)
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
                eprintln!("error: `ql build` expects a file or directory path");
                return Err(1);
            };

            let mut options = BuildOptions::default();
            let mut profile_override = None;
            let mut emit_overridden = false;
            let mut emit_interface = false;
            let mut json = false;
            let mut selector = ProjectTargetSelector::default();
            let remaining = args.collect::<Vec<_>>();
            let mut index = 0;

            while index < remaining.len() {
                if parse_project_target_selector_option(
                    "`ql build`",
                    &remaining,
                    &mut index,
                    &mut selector,
                )? {
                    index += 1;
                    continue;
                }

                match remaining[index].as_str() {
                    "--emit" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --emit` expects a value");
                            return Err(1);
                        };
                        emit_overridden = true;
                        match value.as_str() {
                            "llvm-ir" => options.emit = BuildEmit::LlvmIr,
                            "asm" => options.emit = BuildEmit::Assembly,
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
                        set_cli_build_profile(
                            "`ql build`",
                            &mut profile_override,
                            BuildProfile::Release,
                        )?;
                    }
                    "--profile" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --profile` expects `debug` or `release`");
                            return Err(1);
                        };
                        let parsed = parse_cli_build_profile("`ql build`", value)?;
                        set_cli_build_profile("`ql build`", &mut profile_override, parsed)?;
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
                    "--json" => {
                        json = true;
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

            let profile_overridden = profile_override.is_some();
            if let Some(profile) = profile_override {
                options.profile = profile;
            }
            build_path(
                Path::new(&path),
                &options,
                &selector,
                emit_interface,
                emit_overridden,
                profile_overridden,
                json,
            )
        }
        "run" => {
            let remaining = args.collect::<Vec<_>>();
            let mut path = None;
            let mut profile_override = None;
            let mut selector = ProjectTargetSelector::default();
            let mut program_args = Vec::new();
            let mut passthrough = false;
            let mut index = 0;

            while index < remaining.len() {
                let argument = &remaining[index];
                if passthrough {
                    program_args.push(argument.clone());
                    index += 1;
                    continue;
                }

                if parse_project_target_selector_option(
                    "`ql run`",
                    &remaining,
                    &mut index,
                    &mut selector,
                )? {
                    index += 1;
                    continue;
                }

                match argument.as_str() {
                    "--" => {
                        passthrough = true;
                    }
                    "--release" => {
                        set_cli_build_profile(
                            "`ql run`",
                            &mut profile_override,
                            BuildProfile::Release,
                        )?;
                    }
                    "--profile" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql run --profile` expects `debug` or `release`");
                            return Err(1);
                        };
                        let parsed = parse_cli_build_profile("`ql run`", value)?;
                        set_cli_build_profile("`ql run`", &mut profile_override, parsed)?;
                    }
                    other if other.starts_with('-') => {
                        eprintln!("error: unknown `ql run` option `{other}`");
                        return Err(1);
                    }
                    other => {
                        if path.is_some() {
                            eprintln!("error: unknown `ql run` argument `{other}`");
                            eprintln!(
                                "hint: use `ql run <file-or-dir> -- <args...>` to pass arguments to the built executable"
                            );
                            return Err(1);
                        }
                        path = Some(other.to_owned());
                    }
                }

                index += 1;
            }

            let Some(path) = path else {
                eprintln!("error: `ql run` expects a file or directory path");
                return Err(1);
            };

            run_path(
                Path::new(&path),
                profile_override.unwrap_or_default(),
                profile_override.is_some(),
                &selector,
                &program_args,
            )
        }
        "test" => {
            let remaining = args.collect::<Vec<_>>();
            let mut path = None;
            let mut options = TestCommandOptions::default();
            let mut profile_override = None;
            let mut index = 0;

            while index < remaining.len() {
                match remaining[index].as_str() {
                    "--release" => {
                        set_cli_build_profile(
                            "`ql test`",
                            &mut profile_override,
                            BuildProfile::Release,
                        )?;
                    }
                    "--profile" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql test --profile` expects `debug` or `release`");
                            return Err(1);
                        };
                        let parsed = parse_cli_build_profile("`ql test`", value)?;
                        set_cli_build_profile("`ql test`", &mut profile_override, parsed)?;
                    }
                    "--package" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql test --package` expects a package name");
                            return Err(1);
                        };
                        if options.package_name.is_some() {
                            eprintln!("error: `ql test` received multiple `--package` selectors");
                            return Err(1);
                        }
                        options.package_name = Some(value.to_owned());
                    }
                    "--list" => {
                        options.list_only = true;
                    }
                    "--json" => {
                        options.json = true;
                    }
                    "--filter" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql test --filter` expects a substring");
                            return Err(1);
                        };
                        options.filter = Some(value.to_owned());
                    }
                    "--target" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql test --target` expects a test path");
                            return Err(1);
                        };
                        if options.target_path.is_some() {
                            eprintln!("error: `ql test` received multiple `--target` selectors");
                            return Err(1);
                        }
                        options.target_path = Some(normalize_path(Path::new(value)));
                    }
                    other if other.starts_with('-') => {
                        eprintln!("error: unknown `ql test` option `{other}`");
                        return Err(1);
                    }
                    other => {
                        if path.is_some() {
                            eprintln!("error: unknown `ql test` argument `{other}`");
                            return Err(1);
                        }
                        path = Some(other.to_owned());
                    }
                }

                index += 1;
            }

            let Some(path) = path else {
                eprintln!("error: `ql test` expects a file or directory path");
                return Err(1);
            };

            options.profile = profile_override.unwrap_or_default();
            options.profile_overridden = profile_override.is_some();
            test_path(Path::new(&path), &options)
        }
        "project" => {
            let Some(subcommand) = args.next() else {
                eprintln!("error: `ql project` expects a subcommand");
                return Err(1);
            };

            match subcommand.as_str() {
                "targets" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--json" => {
                                json = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project targets` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project targets` argument `{other}`"
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
                    project_targets_path(&path, json)
                }
                "graph" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--json" => {
                                json = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project graph` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project graph` argument `{other}`"
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
                    project_graph_path(&path, json)
                }
                "lock" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut check_only = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--check" => {
                                check_only = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project lock` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project lock` argument `{other}`"
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
                    project_lock_path(&path, check_only)
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
                "init" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut workspace = false;
                    let mut package_name = None;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--workspace" => {
                                workspace = true;
                            }
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project init --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project init` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project init` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project init` argument `{other}`"
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
                    project_init_path(&path, workspace, package_name.as_deref())
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

fn check_path(path: &Path, sync_interfaces: bool, json: bool) -> Result<(), u8> {
    let use_package_check = should_use_package_check(path)
        || (is_ql_source_file(path) && load_project_manifest(path).is_ok());
    if use_package_check {
        let check_command_label = format_check_command_label(sync_interfaces);
        let mut package_manifest_path = None;
        let mut json_report = None;
        if let Ok(manifest) = load_project_manifest(path) {
            if manifest.package.is_none() && manifest.workspace.is_some() {
                return check_workspace_manifest(&manifest, sync_interfaces, json);
            }
            package_manifest_path = Some(manifest.manifest_path.clone());
            if json {
                json_report = Some(CheckJsonReport::new(
                    "package",
                    sync_interfaces,
                    Some(&manifest.manifest_path),
                ));
            }
            if !sync_interfaces && ensure_reference_interfaces_current(&manifest).is_err() {
                report_package_check_reference_failure(&manifest.manifest_path, sync_interfaces);
                return Err(1);
            }
        }
        if sync_interfaces {
            let synced_paths = match sync_reference_interfaces(path, &mut BTreeSet::new()) {
                Ok(paths) => paths,
                Err(_) => {
                    if let Some(manifest_path) = package_manifest_path.as_deref() {
                        report_package_check_reference_failure(manifest_path, sync_interfaces);
                    }
                    return Err(1);
                }
            };
            for interface_path in synced_paths {
                if let Some(report) = json_report.as_mut() {
                    report.record_written_interface(&interface_path);
                } else {
                    println!("wrote interface: {}", interface_path.display());
                }
            }
        }
        match analyze_package(path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: {check_command_label} no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    report_package_check_no_sources_failure(
                        &package.manifest().manifest_path,
                        &source_root,
                        sync_interfaces,
                    );
                    return Err(1);
                }
                for module in package.modules() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_checked_file(module.path());
                    } else {
                        println!("ok: {}", module.path().display());
                    }
                }
                for dependency in package.dependencies() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_loaded_interface(dependency.interface_path());
                    } else {
                        println!(
                            "loaded interface: {}",
                            dependency.interface_path().display()
                        );
                    }
                }
                if let Some(report) = json_report {
                    print!("{}", report.into_json());
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
            Err(PackageAnalysisError::Project(error)) => {
                if let Some(manifest_path) =
                    package_missing_name_manifest_path_from_project_error(&error)
                {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        check_command_label,
                        normalize_path(manifest_path)
                    );
                    report_package_check_manifest_failure(manifest_path, sync_interfaces);
                } else if let Some(manifest_path) =
                    package_check_manifest_path_from_project_error(&error)
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_package_check_manifest_failure(manifest_path, sync_interfaces);
                } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_package_check_source_root_failure(
                        package_manifest_path
                            .as_deref()
                            .expect("package source root failures require a loaded manifest"),
                        path,
                        sync_interfaces,
                    );
                } else {
                    print_package_analysis_error(&PackageAnalysisError::Project(error));
                }
                return Err(1);
            }
            Err(error) => {
                if let PackageAnalysisError::SourceDiagnostics {
                    path,
                    source,
                    diagnostics,
                } = &error
                {
                    if let Some(mut report) = json_report {
                        report.record_source_diagnostics(
                            path,
                            source,
                            diagnostics,
                            package_manifest_path.as_deref(),
                        );
                        print!("{}", report.into_json());
                        return Err(1);
                    }
                }
                print_package_analysis_error(&error);
                if matches!(&error, PackageAnalysisError::SourceDiagnostics { .. }) {
                    report_package_check_source_diagnostics_failure(
                        package_manifest_path
                            .as_deref()
                            .expect("package source diagnostics require a loaded manifest"),
                        sync_interfaces,
                    );
                }
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
    let mut json_report = json.then(|| CheckJsonReport::new("files", sync_interfaces, None));

    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            eprintln!("error: failed to read `{}`: {error}", file.display());
            1
        })?;

        match analyze_source(&source) {
            Ok(()) => {
                if let Some(report) = json_report.as_mut() {
                    report.record_checked_file(&file);
                } else {
                    println!("ok: {}", file.display());
                }
            }
            Err(diagnostics) => {
                has_errors = true;
                if let Some(report) = json_report.as_mut() {
                    report.record_source_diagnostics(&file, &source, &diagnostics, None);
                } else {
                    print_diagnostics(&file, &source, &diagnostics);
                }
            }
        }
    }

    if let Some(report) = json_report {
        print!("{}", report.into_json());
    }

    if has_errors { Err(1) } else { Ok(()) }
}

fn check_workspace_manifest(
    manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
    json: bool,
) -> Result<(), u8> {
    let Some(workspace) = &manifest.workspace else {
        return Ok(());
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let check_command_label = format_check_command_label(sync_interfaces);
    let mut sync_visited = BTreeSet::new();
    let mut synced_interfaces = BTreeSet::new();
    let mut failing_members = 0usize;
    let mut first_failing_member_manifest = None;
    let mut json_report = json
        .then(|| CheckJsonReport::new("workspace", sync_interfaces, Some(&manifest.manifest_path)));
    let mut json_supported_failure_only = true;

    for member in &workspace.members {
        let member_path = manifest_dir.join(member);
        let member_manifest = match load_project_manifest(&member_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                json_supported_failure_only = false;
                let member_manifest_path = workspace_member_manifest_path(&member_path);
                if let Some(manifest_path) =
                    package_missing_name_manifest_path_from_project_error(&error)
                {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        check_command_label,
                        normalize_path(manifest_path)
                    );
                    report_workspace_member_package_check_manifest_failure(
                        manifest_path,
                        sync_interfaces,
                    );
                } else if let Some(manifest_path) =
                    package_check_manifest_path_from_project_error(&error)
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_workspace_member_package_check_manifest_failure(
                        manifest_path,
                        sync_interfaces,
                    );
                } else {
                    eprintln!("error: {check_command_label} {error}");
                    let rerun_command = format_workspace_member_check_rerun_command(
                        &normalize_path(&member_manifest_path),
                        sync_interfaces,
                    );
                    let rerun_hint = format!(
                        "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                    );
                    report_workspace_member_failure(
                        &member_manifest_path,
                        Some(rerun_hint.as_str()),
                    );
                }
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest_path,
                );
                continue;
            }
        };

        if let Err(error) = package_name(&member_manifest) {
            json_supported_failure_only = false;
            eprintln!("error: {check_command_label} {error}");
            report_workspace_member_package_check_manifest_failure(
                &member_manifest.manifest_path,
                sync_interfaces,
            );
            failing_members += 1;
            record_reference_failure_manifest(
                &mut first_failing_member_manifest,
                member_manifest.manifest_path.clone(),
            );
            continue;
        }

        if !sync_interfaces && ensure_reference_interfaces_current(&member_manifest).is_err() {
            json_supported_failure_only = false;
            report_workspace_member_package_check_reference_failure(
                &member_manifest.manifest_path,
                sync_interfaces,
            );
            failing_members += 1;
            record_reference_failure_manifest(
                &mut first_failing_member_manifest,
                member_manifest.manifest_path.clone(),
            );
            continue;
        }

        if sync_interfaces {
            let synced_paths = match sync_reference_interfaces(&member_path, &mut sync_visited) {
                Ok(paths) => paths,
                Err(_) => {
                    json_supported_failure_only = false;
                    report_workspace_member_package_check_reference_failure(
                        &member_manifest.manifest_path,
                        sync_interfaces,
                    );
                    failing_members += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                    continue;
                }
            };
            for interface_path in synced_paths {
                let display_path =
                    fs::canonicalize(&interface_path).unwrap_or_else(|_| interface_path.clone());
                if synced_interfaces.insert(display_path.clone()) {
                    if let Some(report) = json_report.as_mut() {
                        report.record_written_interface(&display_path);
                    } else {
                        println!("wrote interface: {}", display_path.display());
                    }
                }
            }
        }

        match analyze_package(&member_path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    json_supported_failure_only = false;
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: {check_command_label} no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    report_workspace_member_package_check_no_sources_failure(
                        &member_manifest.manifest_path,
                        &source_root,
                        sync_interfaces,
                    );
                    failing_members += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                    continue;
                }
                for module in package.modules() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_checked_file(module.path());
                    } else {
                        println!("ok: {}", module.path().display());
                    }
                }
                for dependency in package.dependencies() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_loaded_interface(dependency.interface_path());
                    } else {
                        println!(
                            "loaded interface: {}",
                            dependency.interface_path().display()
                        );
                    }
                }
            }
            Err(PackageAnalysisError::Project(
                ql_project::ProjectError::PackageSourceRootNotFound { path },
            )) => {
                json_supported_failure_only = false;
                eprintln!(
                    "error: {check_command_label} package source directory `{}` does not exist",
                    normalize_path(&path)
                );
                report_workspace_member_package_check_source_root_failure(
                    &member_manifest.manifest_path,
                    &path,
                    sync_interfaces,
                );
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
            }
            Err(error) => {
                if let PackageAnalysisError::SourceDiagnostics {
                    path,
                    source,
                    diagnostics,
                } = &error
                {
                    if let Some(report) = json_report.as_mut() {
                        report.record_source_diagnostics(
                            path,
                            source,
                            diagnostics,
                            Some(&member_manifest.manifest_path),
                        );
                        failing_members += 1;
                        record_reference_failure_manifest(
                            &mut first_failing_member_manifest,
                            member_manifest.manifest_path.clone(),
                        );
                        continue;
                    }
                }
                json_supported_failure_only = false;
                print_package_analysis_error(&error);
                if matches!(&error, PackageAnalysisError::SourceDiagnostics { .. }) {
                    report_workspace_member_package_check_source_diagnostics_failure(
                        &member_manifest.manifest_path,
                        sync_interfaces,
                    );
                } else {
                    report_workspace_member_failure(&member_manifest.manifest_path, None);
                }
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
            }
        }
    }

    if json_supported_failure_only {
        if let Some(report) = json_report {
            print!("{}", report.into_json());
        }
        if failing_members > 0 {
            return Err(1);
        }
        return Ok(());
    }

    if failing_members > 0 {
        eprintln!("error: {check_command_label} found {failing_members} failing member(s)");
        if failing_members > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    Ok(())
}

fn report_workspace_member_failure(manifest_path: &Path, hint_line: Option<&str>) {
    eprintln!(
        "note: failing workspace member manifest: {}",
        normalize_path(manifest_path)
    );
    if let Some(hint_line) = hint_line {
        eprintln!("{hint_line}");
    }
}

fn format_workspace_member_reference_failure_rerun_hint(
    manifest_path: &Path,
    sync_interfaces: bool,
) -> String {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    format!(
        "hint: rerun `{rerun_command}` after fixing the referenced package or reference manifest"
    )
}

fn report_workspace_member_package_check_source_root_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
}

fn report_workspace_member_package_check_no_sources_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
}

fn report_workspace_member_package_check_source_diagnostics_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package sources");
}

fn report_workspace_member_package_check_reference_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_hint = format_workspace_member_reference_failure_rerun_hint(
        Path::new(&manifest_path),
        sync_interfaces,
    );
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("{rerun_hint}");
}

fn report_workspace_member_package_check_manifest_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_workspace_member_package_interface_check_manifest_failure(
    manifest_path: &Path,
    changed_only: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_emit_rerun_command(&manifest_path, changed_only, true);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_graph_manifest_failure(manifest_path: &Path) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format!("ql project graph {manifest_path}");
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_graph_package_context_failure(path: &Path) {
    let normalized_path = normalize_path(path);
    let rerun_command = format!("ql project graph {normalized_path}");
    eprintln!(
        "note: `ql project graph` only renders package/workspace graphs for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

fn format_project_lock_command(manifest_path: &Path, check_only: bool) -> String {
    let manifest_path = normalize_path(manifest_path);
    if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    }
}

fn report_project_lock_manifest_failure(manifest_path: &Path, check_only: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    };
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_lock_package_context_failure(path: &Path, check_only: bool) {
    let normalized_path = normalize_path(path);
    let rerun_command = if check_only {
        format!("ql project lock {normalized_path} --check")
    } else {
        format!("ql project lock {normalized_path}")
    };
    eprintln!(
        "note: `ql project lock` only writes or checks package/workspace lockfiles for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

fn report_package_interface_check_manifest_failure(manifest_path: &Path, changed_only: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_emit_rerun_command(&manifest_path, changed_only, true);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn package_check_manifest_path_from_project_error(
    error: &ql_project::ProjectError,
) -> Option<&Path> {
    match error {
        ql_project::ProjectError::PackageNotDefined { path }
        | ql_project::ProjectError::Read { path, .. }
        | ql_project::ProjectError::Parse { path, .. } => Some(path.as_path()),
        ql_project::ProjectError::ManifestNotFound { .. }
        | ql_project::ProjectError::PackageSourceRootNotFound { .. } => None,
    }
}

fn package_missing_name_manifest_path_from_project_error(
    error: &ql_project::ProjectError,
) -> Option<&Path> {
    match error {
        ql_project::ProjectError::PackageNotDefined { path } => Some(path.as_path()),
        ql_project::ProjectError::Parse { path, message }
            if message == "`[package].name` must be present" =>
        {
            Some(path.as_path())
        }
        _ => None,
    }
}

fn report_package_check_manifest_failure(manifest_path: &Path, sync_interfaces: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_package_check_source_root_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
}

fn report_package_check_no_sources_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
}

fn report_package_check_source_diagnostics_failure(manifest_path: &Path, sync_interfaces: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package sources");
}

fn report_package_check_reference_failure(manifest_path: &Path, sync_interfaces: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "hint: rerun `{rerun_command}` after fixing the referenced package or reference manifest"
    );
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

fn build_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
) -> Result<(), u8> {
    if should_use_project_build(path) {
        return build_project_path(
            path,
            options,
            selector,
            emit_interface,
            emit_overridden,
            profile_overridden,
            json,
        );
    }

    if let Some(request) = resolve_project_source_build_target_request(path) {
        if selector.is_active() {
            report_project_source_path_rejects_target_selector("`ql build`", path, selector);
            return Err(1);
        }
        return build_project_path(
            path,
            options,
            &request.selector,
            emit_interface,
            emit_overridden,
            profile_overridden,
            json,
        );
    }

    if selector.is_active() {
        if json {
            let mut report =
                BuildJsonReport::new(path, options, profile_overridden, emit_interface);
            report.record_preflight_failure(build_json_preflight_failure(
                path,
                None,
                None,
                None,
                "selector",
                "project-context",
                "target selectors require a package or workspace path".to_owned(),
                Some(selector.describe()),
                None,
                None,
            ));
            print!("{}", report.into_json());
        } else {
            report_project_target_selector_requires_project_context("`ql build`", selector);
        }
        return Err(1);
    }

    if json {
        let mut report = BuildJsonReport::new(path, options, profile_overridden, emit_interface);
        let artifact = match build_single_source_target_result(path, options) {
            Ok(artifact) => artifact,
            Err(error) => {
                report.record_source_failure(path, &error);
                print!("{}", report.into_json());
                return Err(1);
            }
        };
        report.record_source_target(path, &artifact);
        if emit_interface {
            match emit_built_package_interface_quiet(path, options, &artifact.path, &[]) {
                Ok(interface_result) => {
                    report.record_interface_result(None, None, true, interface_result);
                }
                Err(error) => {
                    report.record_preflight_failure(build_json_emit_interface_failure(
                        path, None, None, &error,
                    ));
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        }
        print!("{}", report.into_json());
        return Ok(());
    }

    let artifact = build_single_source_target(path, options, emit_interface)?;
    if emit_interface {
        emit_built_package_interface(path, options, &artifact.path, &[])?;
    }
    Ok(())
}

#[derive(Debug)]
struct BuildJsonReport {
    scope: &'static str,
    path: String,
    project_manifest_path: Option<String>,
    requested_emit: &'static str,
    requested_profile: &'static str,
    profile_overridden: bool,
    emit_interface: bool,
    built_targets: Vec<JsonValue>,
    interfaces: Vec<JsonValue>,
    failure: Option<JsonValue>,
}

impl BuildJsonReport {
    fn new(
        path: &Path,
        options: &BuildOptions,
        profile_overridden: bool,
        emit_interface: bool,
    ) -> Self {
        let direct_source_request = resolve_project_source_build_target_request(path);
        let project_scope = should_use_project_build(path) || direct_source_request.is_some();
        let project_manifest_path = if let Some(request) = direct_source_request.as_ref() {
            Some(normalize_path(&request.manifest_path))
        } else if project_scope {
            load_project_manifest(path)
                .ok()
                .map(|manifest| normalize_path(&manifest.manifest_path))
        } else {
            None
        };
        Self {
            scope: if project_scope { "project" } else { "file" },
            path: normalize_path(path),
            project_manifest_path,
            requested_emit: build_emit_cli_value(options.emit),
            requested_profile: options.profile.dir_name(),
            profile_overridden,
            emit_interface,
            built_targets: Vec::new(),
            interfaces: Vec::new(),
            failure: None,
        }
    }

    fn record_source_target(&mut self, path: &Path, artifact: &BuildArtifact) {
        self.built_targets.push(build_json_target(
            None,
            None,
            "source",
            normalize_path(path),
            artifact,
            true,
        ));
    }

    fn record_project_target(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        artifact: &BuildArtifact,
        selected: bool,
    ) {
        self.built_targets.push(build_json_target(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            artifact,
            selected,
        ));
    }

    fn record_source_failure(&mut self, path: &Path, error: &BuildError) {
        self.failure = Some(build_json_failure(
            None,
            None,
            "source",
            normalize_path(path),
            true,
            error,
        ));
    }

    fn record_preflight_failure(&mut self, failure: JsonValue) {
        self.failure = Some(failure);
    }

    fn record_project_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        error: &BuildError,
        selected: bool,
    ) {
        self.failure = Some(build_json_failure(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            selected,
            error,
        ));
    }

    fn record_interface_result(
        &mut self,
        manifest_path: Option<&Path>,
        package_name: Option<&str>,
        selected: bool,
        result: EmitPackageInterfaceResult,
    ) {
        let (status, path) = match result {
            EmitPackageInterfaceResult::Wrote(path) => ("wrote", path),
            EmitPackageInterfaceResult::UpToDate(path) => ("up-to-date", path),
        };
        self.interfaces.push(json!({
            "manifest_path": manifest_path.map(normalize_path),
            "package_name": package_name,
            "selected": selected,
            "status": status,
            "path": normalize_path(&path),
        }));
    }

    fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.build.v1",
            "path": self.path,
            "scope": self.scope,
            "project_manifest_path": self.project_manifest_path,
            "requested_emit": self.requested_emit,
            "requested_profile": self.requested_profile,
            "profile_overridden": self.profile_overridden,
            "emit_interface": self.emit_interface,
            "status": if self.failure.is_some() { "failed" } else { "ok" },
            "built_targets": self.built_targets,
            "interfaces": self.interfaces,
            "failure": self.failure,
        }))
        .expect("build json report should serialize");
        format!("{rendered}\n")
    }
}

fn build_json_target(
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    kind: &str,
    display_path: String,
    artifact: &BuildArtifact,
    selected: bool,
) -> JsonValue {
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": selected,
        "dependency_only": !selected,
        "kind": kind,
        "path": display_path,
        "emit": build_emit_cli_value(artifact.emit),
        "profile": artifact.profile.dir_name(),
        "artifact_path": normalize_path(&artifact.path),
        "c_header_path": artifact.c_header.as_ref().map(|header| normalize_path(&header.path)),
    })
}

fn build_json_failure(
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    kind: &str,
    display_path: String,
    selected: bool,
    error: &BuildError,
) -> JsonValue {
    let manifest_path = manifest_path.map(normalize_path);
    match error {
        BuildError::InvalidInput(message) => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "invalid-input",
            "message": message,
        }),
        BuildError::Io { path, error } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "io",
            "message": format!("failed to access `{}`: {error}", normalize_path(path)),
            "io_path": normalize_path(path),
        }),
        BuildError::Toolchain {
            error,
            preserved_artifacts,
        } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "toolchain",
            "message": error.to_string(),
            "preserved_artifacts": preserved_artifacts
                .iter()
                .map(|path| normalize_path(path))
                .collect::<Vec<_>>(),
            "intermediate_ir": preserved_artifacts
                .iter()
                .find(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains(".codegen.ll"))
                })
                .map(|path| normalize_path(path)),
        }),
        BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "diagnostics",
            "message": "build produced diagnostics",
            "diagnostic_file": build_json_diagnostic_file(path, source, diagnostics),
        }),
    }
}

fn build_json_preflight_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    selected: Option<bool>,
    error_kind: &str,
    stage: &str,
    message: String,
    selector: Option<String>,
    conflict_path: Option<String>,
    target_count: Option<usize>,
) -> JsonValue {
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": selected,
        "dependency_only": selected.map(|value| !value),
        "kind": JsonValue::Null,
        "path": normalize_path(request_path),
        "error_kind": error_kind,
        "stage": stage,
        "message": message,
        "selector": selector,
        "conflict_path": conflict_path,
        "target_count": target_count,
    })
}

fn build_json_project_error(
    request_path: &Path,
    error: &ql_project::ProjectError,
    stage: &str,
) -> JsonValue {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return build_json_preflight_failure(
            request_path,
            None,
            None,
            None,
            "manifest",
            stage,
            format!(
                "could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            ),
            None,
            None,
            None,
        );
    }

    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return build_json_preflight_failure(
            request_path,
            Some(manifest_path),
            None,
            None,
            "manifest",
            stage,
            format!(
                "manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            ),
            None,
            None,
            None,
        );
    }

    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return build_json_preflight_failure(
            request_path,
            None,
            None,
            None,
            "manifest",
            stage,
            format!(
                "package source directory `{}` does not exist",
                normalize_path(path)
            ),
            None,
            None,
            None,
        );
    }

    build_json_preflight_failure(
        request_path,
        package_check_manifest_path_from_project_error(error),
        None,
        None,
        "manifest",
        stage,
        error.to_string(),
        None,
        None,
        None,
    )
}

fn build_json_interface_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    error_kind: &str,
    message: String,
    output_path: Option<String>,
    source_root: Option<String>,
    failing_source_count: Option<usize>,
    first_failing_source: Option<String>,
) -> JsonValue {
    let display_path = output_path
        .clone()
        .or_else(|| manifest_path.map(normalize_path))
        .unwrap_or_else(|| normalize_path(request_path));
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": true,
        "dependency_only": false,
        "kind": "interface",
        "path": display_path,
        "error_kind": error_kind,
        "stage": "emit-interface",
        "message": message,
        "output_path": output_path,
        "source_root": source_root,
        "failing_source_count": failing_source_count,
        "first_failing_source": first_failing_source,
    })
}

fn build_json_emit_interface_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    error: &EmitPackageInterfaceError,
) -> JsonValue {
    match error {
        EmitPackageInterfaceError::ManifestNotFound { start } => build_json_interface_failure(
            request_path,
            None,
            package_name,
            "project-context",
            format!(
                "build-side interface emission requires a package manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            ),
            None,
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::ManifestFailure {
            manifest_path,
            message,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "manifest",
            message.clone(),
            None,
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path,
            source_root,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "package-sources",
            format!(
                "no `.ql` files found under `{}`",
                normalize_path(source_root)
            ),
            None,
            Some(normalize_path(source_root)),
            None,
            None,
        ),
        EmitPackageInterfaceError::SourceRootFailure {
            manifest_path,
            source_root,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "package-source-root",
            format!(
                "package source directory `{}` does not exist",
                normalize_path(source_root)
            ),
            None,
            Some(normalize_path(source_root)),
            None,
            None,
        ),
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: output_manifest_path,
            output_path,
            message,
        } => build_json_interface_failure(
            request_path,
            output_manifest_path.as_deref().or(manifest_path),
            package_name,
            "interface-output",
            message.clone(),
            Some(normalize_path(output_path)),
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::SourceFailure {
            failure_count,
            first_failing_source,
            ..
        } => build_json_interface_failure(
            request_path,
            manifest_path,
            package_name,
            "package-sources",
            format!("package interface emission found {failure_count} failing source file(s)"),
            None,
            None,
            Some(*failure_count),
            first_failing_source
                .as_ref()
                .map(|path| normalize_path(path)),
        ),
        EmitPackageInterfaceError::Code { message, .. } => build_json_interface_failure(
            request_path,
            manifest_path,
            package_name,
            "interface",
            message
                .clone()
                .unwrap_or_else(|| "build-side interface emission failed".to_owned()),
            None,
            None,
            None,
            None,
        ),
    }
}

fn build_json_dependency_interface_prep_failure(
    _request_path: &Path,
    failure: &ReferenceInterfacePrepError,
) -> JsonValue {
    let manifest_path = failure.first_failure.manifest_path.as_deref().or(Some(
        failure.first_failure.reference_manifest_path.as_path(),
    ));
    let reference_manifest_path = normalize_path(&failure.first_failure.reference_manifest_path);
    let owner_manifest_path = failure
        .first_failure
        .owner_manifest_path
        .as_ref()
        .map(|path| normalize_path(path));
    let first_failing_dependency_manifest = failure
        .first_failure_manifest
        .as_ref()
        .map(|path| normalize_path(path));
    let (error_kind, message, output_path, source_root, failing_source_count, first_failing_source) =
        match &failure.first_failure.failure_kind {
            ReferenceInterfacePrepFailureKind::Project {
                error_kind,
                message,
                source_root,
            } => (
                *error_kind,
                message.clone(),
                None,
                source_root.as_ref().map(|path| normalize_path(path)),
                None,
                None,
            ),
            ReferenceInterfacePrepFailureKind::InterfaceEmit(error) => match error {
                EmitPackageInterfaceError::ManifestNotFound { start } => (
                    "project-context",
                    format!(
                        "could not find `qlang.toml` starting from `{}`",
                        normalize_path(start)
                    ),
                    None,
                    None,
                    None,
                    None,
                ),
                EmitPackageInterfaceError::ManifestFailure { message, .. } => {
                    ("manifest", message.clone(), None, None, None, None)
                }
                EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => (
                    "package-sources",
                    format!(
                        "no `.ql` files found under `{}`",
                        normalize_path(source_root)
                    ),
                    None,
                    Some(normalize_path(source_root)),
                    None,
                    None,
                ),
                EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => (
                    "package-source-root",
                    format!(
                        "package source directory `{}` does not exist",
                        normalize_path(source_root)
                    ),
                    None,
                    Some(normalize_path(source_root)),
                    None,
                    None,
                ),
                EmitPackageInterfaceError::OutputPathFailure {
                    output_path,
                    message,
                    ..
                } => (
                    "interface-output",
                    message.clone(),
                    Some(normalize_path(output_path)),
                    None,
                    None,
                    None,
                ),
                EmitPackageInterfaceError::SourceFailure {
                    failure_count,
                    first_failing_source,
                    ..
                } => (
                    "package-sources",
                    format!(
                        "package interface emission found {failure_count} failing source file(s)"
                    ),
                    None,
                    None,
                    Some(*failure_count),
                    first_failing_source
                        .as_ref()
                        .map(|path| normalize_path(path)),
                ),
                EmitPackageInterfaceError::Code { message, .. } => (
                    "interface",
                    message
                        .clone()
                        .unwrap_or_else(|| "dependency interface preparation failed".to_owned()),
                    None,
                    None,
                    None,
                    None,
                ),
            },
        };
    let display_path = output_path
        .clone()
        .or_else(|| source_root.clone())
        .or_else(|| manifest_path.map(normalize_path))
        .unwrap_or_else(|| reference_manifest_path.clone());
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": JsonValue::Null,
        "selected": false,
        "dependency_only": true,
        "kind": "interface",
        "path": display_path,
        "error_kind": error_kind,
        "stage": "dependency-interface-prep",
        "message": message,
        "output_path": output_path,
        "source_root": source_root,
        "failing_source_count": failing_source_count,
        "first_failing_source": first_failing_source,
        "owner_manifest_path": owner_manifest_path,
        "reference_manifest_path": reference_manifest_path,
        "reference": failure.first_failure.reference.clone(),
        "failing_dependency_count": failure.failure_count,
        "first_failing_dependency_manifest": first_failing_dependency_manifest,
    })
}

fn build_json_build_plan_failure(
    request_path: &Path,
    failure: &BuildPlanResolveError,
) -> JsonValue {
    match &failure.failure_kind {
        BuildPlanResolveFailureKind::Dependency { message } => json!({
            "manifest_path": failure.manifest_path.as_ref().map(|path| normalize_path(path)),
            "package_name": JsonValue::Null,
            "selected": JsonValue::Null,
            "dependency_only": JsonValue::Null,
            "kind": JsonValue::Null,
            "path": normalize_path(request_path),
            "error_kind": "dependency",
            "stage": "build-plan",
            "message": message,
            "owner_manifest_path": failure.owner_manifest_path.as_ref().map(|path| normalize_path(path)),
            "dependency_manifest_path": failure.dependency_manifest_path.as_ref().map(|path| normalize_path(path)),
            "cycle_manifests": JsonValue::Null,
        }),
        BuildPlanResolveFailureKind::Cycle { cycle_manifests } => json!({
            "manifest_path": failure.manifest_path.as_ref().map(|path| normalize_path(path)),
            "package_name": JsonValue::Null,
            "selected": JsonValue::Null,
            "dependency_only": JsonValue::Null,
            "kind": JsonValue::Null,
            "path": normalize_path(request_path),
            "error_kind": "cycle",
            "stage": "build-plan",
            "message": "local package build dependencies contain a cycle",
            "owner_manifest_path": JsonValue::Null,
            "dependency_manifest_path": failure.dependency_manifest_path.as_ref().map(|path| normalize_path(path)),
            "cycle_manifests": cycle_manifests,
        }),
    }
}

fn build_json_target_prep_failure(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    selected: bool,
    failure: &PrepareProjectTargetBuildError,
) -> JsonValue {
    let (
        error_kind,
        message,
        dependency_manifest_path,
        dependency_package,
        interface_path,
        symbol,
        first_dependency_package,
        first_dependency_manifest_path,
        conflicting_dependency_package,
        conflicting_dependency_manifest_path,
        io_path,
    ) = match &failure.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyManifest {
            dependency_manifest_path,
            error_kind,
            message,
        } => (
            *error_kind,
            message.clone(),
            dependency_manifest_path
                .as_ref()
                .map(|path| json!(normalize_path(path)))
                .unwrap_or(JsonValue::Null),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyInterface {
            dependency_manifest_path,
            dependency_package,
            interface_path,
            message,
        } => (
            "dependency-interface",
            message.clone(),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            json!(normalize_path(interface_path)),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-extern-conflict",
            format!("found conflicting direct dependency extern imports for `{symbol}`"),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(symbol),
            json!(first_package),
            json!(normalize_path(first_manifest_path)),
            json!(conflicting_package),
            json!(normalize_path(conflicting_manifest_path)),
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::SourceRead { path, message } => (
            "io",
            message.clone(),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(normalize_path(path)),
        ),
    };

    json!({
        "manifest_path": normalize_path(&member.member_manifest_path),
        "package_name": member.package_name,
        "selected": selected,
        "dependency_only": !selected,
        "kind": target.kind.as_str(),
        "path": project_target_display_path(&member.member_manifest_path, &target.path),
        "error_kind": error_kind,
        "stage": "target-prep",
        "message": message,
        "dependency_manifest_path": dependency_manifest_path,
        "dependency_package": dependency_package,
        "interface_path": interface_path,
        "symbol": symbol,
        "first_dependency_package": first_dependency_package,
        "first_dependency_manifest_path": first_dependency_manifest_path,
        "conflicting_dependency_package": conflicting_dependency_package,
        "conflicting_dependency_manifest_path": conflicting_dependency_manifest_path,
        "io_path": io_path,
    })
}

fn emit_build_json_failure(
    json_report: &mut Option<BuildJsonReport>,
    failure: JsonValue,
) -> Result<(), u8> {
    let mut report = json_report
        .take()
        .expect("json report should exist for `ql build --json` failure reporting");
    report.record_preflight_failure(failure);
    print!("{}", report.into_json());
    Err(1)
}

fn load_workspace_build_targets_for_build_json(
    path: &Path,
) -> Result<Vec<WorkspaceBuildTargets>, JsonValue> {
    let manifest = load_project_manifest(path)
        .map_err(|error| build_json_project_error(path, &error, "manifest-load"))?;
    discover_workspace_build_targets(&manifest)
        .map_err(|error| build_json_project_error(path, &error, "target-discovery"))
}

fn select_workspace_build_targets_for_build_json(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, JsonValue> {
    if !selector.is_active() {
        return Ok(members.to_vec());
    }

    let mut selected = Vec::new();
    for member in members {
        let targets = member
            .targets
            .iter()
            .filter(|target| {
                selector.matches(
                    member.member_manifest_path.as_path(),
                    &member.package_name,
                    target,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            selected.push(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            });
        }
    }

    if selected
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>()
        == 0
    {
        let normalized_path = normalize_path(path);
        return Err(build_json_preflight_failure(
            path,
            None,
            None,
            None,
            "selector",
            "target-selection",
            format!("target selector matched no {target_label} under `{normalized_path}`"),
            Some(selector.describe()),
            None,
            Some(0),
        ));
    }

    Ok(selected)
}

fn build_json_diagnostic_file(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "diagnostics": diagnostics
            .iter()
            .map(|diagnostic| check_json_diagnostic(source, diagnostic))
            .collect::<Vec<_>>(),
    })
}

#[derive(Clone, Debug)]
struct RunnableProjectTarget {
    member_manifest_path: PathBuf,
    package_name: String,
    default_profile: Option<ManifestBuildProfile>,
    target: BuildTarget,
}

#[derive(Debug)]
struct CheckJsonReport {
    scope: &'static str,
    sync_interfaces: bool,
    project_manifest_path: Option<String>,
    checked_files: Vec<String>,
    loaded_interfaces: Vec<String>,
    written_interfaces: Vec<String>,
    diagnostic_files: Vec<JsonValue>,
    failing_manifests: Vec<String>,
}

impl CheckJsonReport {
    fn new(
        scope: &'static str,
        sync_interfaces: bool,
        project_manifest_path: Option<&Path>,
    ) -> Self {
        Self {
            scope,
            sync_interfaces,
            project_manifest_path: project_manifest_path.map(normalize_path),
            checked_files: Vec::new(),
            loaded_interfaces: Vec::new(),
            written_interfaces: Vec::new(),
            diagnostic_files: Vec::new(),
            failing_manifests: Vec::new(),
        }
    }

    fn record_checked_file(&mut self, path: &Path) {
        self.checked_files.push(normalize_path(path));
    }

    fn record_loaded_interface(&mut self, path: &Path) {
        self.loaded_interfaces.push(normalize_path(path));
    }

    fn record_written_interface(&mut self, path: &Path) {
        self.written_interfaces.push(normalize_path(path));
    }

    fn record_source_diagnostics(
        &mut self,
        path: &Path,
        source: &str,
        diagnostics: &[Diagnostic],
        owner_manifest_path: Option<&Path>,
    ) {
        if let Some(owner_manifest_path) = owner_manifest_path {
            let manifest_path = normalize_path(owner_manifest_path);
            if !self
                .failing_manifests
                .iter()
                .any(|existing| existing == &manifest_path)
            {
                self.failing_manifests.push(manifest_path);
            }
        }
        self.diagnostic_files.push(check_json_diagnostic_file(
            path,
            source,
            diagnostics,
            owner_manifest_path,
        ));
    }

    fn into_json(self) -> String {
        let status = if self.diagnostic_files.is_empty() {
            "ok"
        } else {
            "diagnostics"
        };
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.check.v1",
            "scope": self.scope,
            "sync_interfaces": self.sync_interfaces,
            "project_manifest_path": self.project_manifest_path,
            "status": status,
            "checked_files": self.checked_files,
            "loaded_interfaces": self.loaded_interfaces,
            "written_interfaces": self.written_interfaces,
            "diagnostic_files": self.diagnostic_files,
            "failing_manifests": self.failing_manifests,
        }))
        .expect("check json report should serialize");
        format!("{rendered}\n")
    }
}

fn check_json_diagnostic_file(
    path: &Path,
    source: &str,
    diagnostics: &[Diagnostic],
    owner_manifest_path: Option<&Path>,
) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "owner_manifest_path": owner_manifest_path.map(normalize_path),
        "diagnostics": diagnostics
            .iter()
            .map(|diagnostic| check_json_diagnostic(source, diagnostic))
            .collect::<Vec<_>>(),
    })
}

fn check_json_diagnostic(source: &str, diagnostic: &Diagnostic) -> JsonValue {
    json!({
        "severity": diagnostic.severity.as_str(),
        "message": diagnostic.message,
        "labels": diagnostic
            .labels
            .iter()
            .map(|label| check_json_label(source, label))
            .collect::<Vec<_>>(),
        "notes": diagnostic.notes,
    })
}

fn check_json_label(source: &str, label: &ql_diagnostics::Label) -> JsonValue {
    let location = locate(source, label.span);
    json!({
        "is_primary": label.is_primary,
        "message": label.message,
        "span": {
            "start_offset": label.span.start,
            "end_offset": label.span.end,
            "start": {
                "line": location.start.line,
                "column": location.start.column,
            },
            "end": {
                "line": location.end.line,
                "column": location.end.column,
            },
        },
    })
}

fn parse_cli_build_profile(command_label: &str, value: &str) -> Result<BuildProfile, u8> {
    match value {
        "debug" => Ok(BuildProfile::Debug),
        "release" => Ok(BuildProfile::Release),
        other => {
            eprintln!("error: {command_label} unsupported profile `{other}`");
            eprintln!("hint: supported profiles are `debug` and `release`");
            Err(1)
        }
    }
}

fn set_cli_build_profile(
    command_label: &str,
    current: &mut Option<BuildProfile>,
    profile: BuildProfile,
) -> Result<(), u8> {
    if current.is_some() {
        eprintln!("error: {command_label} received multiple profile selectors");
        return Err(1);
    }
    *current = Some(profile);
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct ProjectTargetSelector {
    package_name: Option<String>,
    target: Option<ProjectTargetSelectorKind>,
}

#[derive(Clone, Debug)]
enum ProjectTargetSelectorKind {
    Library,
    Binary(String),
    DisplayPath(String),
}

impl ProjectTargetSelector {
    fn is_active(&self) -> bool {
        self.package_name.is_some() || self.target.is_some()
    }

    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(package_name) = self.package_name.as_ref() {
            parts.push(format!("package `{package_name}`"));
        }
        if let Some(target) = self.target.as_ref() {
            parts.push(match target {
                ProjectTargetSelectorKind::Library => "library target".to_owned(),
                ProjectTargetSelectorKind::Binary(name) => format!("binary `{name}`"),
                ProjectTargetSelectorKind::DisplayPath(path) => format!("target `{path}`"),
            });
        }
        parts.join(", ")
    }

    fn matches(&self, manifest_path: &Path, package_name: &str, target: &BuildTarget) -> bool {
        if self
            .package_name
            .as_ref()
            .is_some_and(|expected| expected != package_name)
        {
            return false;
        }

        match self.target.as_ref() {
            None => true,
            Some(ProjectTargetSelectorKind::Library) => target.kind == BuildTargetKind::Library,
            Some(ProjectTargetSelectorKind::Binary(name)) => {
                target.kind == BuildTargetKind::Binary
                    && target
                        .path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem == name)
            }
            Some(ProjectTargetSelectorKind::DisplayPath(path)) => {
                project_target_display_path(manifest_path, target.path.as_path()) == *path
            }
        }
    }
}

fn parse_project_target_selector_option(
    command_label: &str,
    remaining: &[String],
    index: &mut usize,
    selector: &mut ProjectTargetSelector,
) -> Result<bool, u8> {
    match remaining[*index].as_str() {
        "--package" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --package expects a package name");
                return Err(1);
            };
            if selector.package_name.is_some() {
                eprintln!("error: {command_label} received multiple `--package` selectors");
                return Err(1);
            }
            selector.package_name = Some(value.to_owned());
            Ok(true)
        }
        "--lib" => {
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::Library,
            )?;
            Ok(true)
        }
        "--bin" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --bin expects a target name");
                return Err(1);
            };
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::Binary(value.to_owned()),
            )?;
            Ok(true)
        }
        "--target" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --target expects a target path");
                return Err(1);
            };
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::DisplayPath(normalize_path(Path::new(value))),
            )?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn set_project_target_selector_kind(
    command_label: &str,
    selector: &mut ProjectTargetSelector,
    kind: ProjectTargetSelectorKind,
) -> Result<(), u8> {
    if selector.target.is_some() {
        eprintln!(
            "error: {command_label} does not support combining `--lib`, `--bin`, and `--target`"
        );
        return Err(1);
    }
    selector.target = Some(kind);
    Ok(())
}

fn report_project_target_selector_requires_project_context(
    command_label: &str,
    selector: &ProjectTargetSelector,
) {
    eprintln!("error: {command_label} target selectors require a package or workspace path");
    eprintln!("note: selector: {}", selector.describe());
}

fn report_project_source_path_rejects_target_selector(
    command_label: &str,
    path: &Path,
    selector: &ProjectTargetSelector,
) {
    eprintln!(
        "error: {command_label} does not support combining a direct project source path with target selectors"
    );
    eprintln!("note: source path: {}", normalize_path(path));
    eprintln!("note: selector: {}", selector.describe());
}

fn select_workspace_build_targets(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    command_label: &str,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    if !selector.is_active() {
        return Ok(members.to_vec());
    }

    let mut selected = Vec::new();
    for member in members {
        let targets = member
            .targets
            .iter()
            .filter(|target| {
                selector.matches(
                    member.member_manifest_path.as_path(),
                    &member.package_name,
                    target,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            selected.push(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            });
        }
    }

    if selected
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>()
        == 0
    {
        let normalized_path = normalize_path(path);
        eprintln!(
            "error: {command_label} target selector matched no {target_label} under `{normalized_path}`"
        );
        eprintln!("note: selector: {}", selector.describe());
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }

    Ok(selected)
}

fn run_path(
    path: &Path,
    profile: BuildProfile,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
) -> Result<(), u8> {
    let options = run_build_options(profile);
    if should_use_project_build(path) {
        return run_project_path(path, &options, profile_overridden, selector, program_args);
    }

    if let Some(request) = resolve_project_source_build_target_request(path) {
        if selector.is_active() {
            report_project_source_path_rejects_target_selector("`ql run`", path, selector);
            return Err(1);
        }
        return run_project_path(
            path,
            &options,
            profile_overridden,
            &request.selector,
            program_args,
        );
    }

    if selector.is_active() {
        report_project_target_selector_requires_project_context("`ql run`", selector);
        return Err(1);
    }

    let artifact = build_single_source_target_silent(path, &options, false)?;
    run_built_executable(&artifact.path, program_args)
}

fn run_build_options(profile: BuildProfile) -> BuildOptions {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = profile;
    options
}

fn run_project_path(
    path: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
) -> Result<(), u8> {
    let all_members = load_workspace_build_targets_for_command(path, "`ql run`")?;
    let members =
        select_workspace_build_targets(path, &all_members, selector, "`ql run`", "build targets")?;
    let runnable = select_runnable_project_target(path, &members, selector)?;
    prepare_reference_interfaces_for_manifests(
        std::slice::from_ref(&runnable.member_manifest_path),
        "`ql run`",
        false,
    )?;
    let runnable_members = select_project_build_plan_root_members(
        &all_members,
        std::slice::from_ref(&runnable.member_manifest_path),
    );
    prepare_project_dependency_builds(
        &all_members,
        &runnable_members,
        "`ql run`",
        options,
        profile_overridden,
    )?;
    let mut target_options =
        apply_manifest_default_profile(options, runnable.default_profile, profile_overridden);
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &runnable.member_manifest_path,
            runnable.target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }
    let artifact = build_project_source_target_silent(
        &all_members,
        "`ql run`",
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
    )?;
    run_built_executable(&artifact.path, program_args)
}

fn select_runnable_project_target(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, u8> {
    let mut runnable_targets = Vec::new();
    for member in members {
        for target in &member.targets {
            if is_runnable_project_target(target.kind) {
                runnable_targets.push(RunnableProjectTarget {
                    member_manifest_path: member.member_manifest_path.clone(),
                    package_name: member.package_name.clone(),
                    default_profile: member.default_profile,
                    target: target.clone(),
                });
            }
        }
    }

    match runnable_targets.len() {
        0 => {
            let normalized_path = normalize_path(path);
            if selector.is_active() {
                eprintln!(
                    "error: `ql run` target selector matched no runnable build targets under `{normalized_path}`"
                );
                eprintln!("note: selector: {}", selector.describe());
                eprintln!(
                    "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
                );
                return Err(1);
            }
            eprintln!("error: `ql run` found no runnable build targets under `{normalized_path}`");
            eprintln!(
                "hint: add `src/main.ql`, `src/bin/*.ql`, or declare `[[bin]].path`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
            );
            Err(1)
        }
        1 => Ok(runnable_targets
            .pop()
            .expect("runnable target count checked above")),
        count => {
            let normalized_path = normalize_path(path);
            if selector.is_active() {
                eprintln!(
                    "error: `ql run` target selector matched multiple runnable build targets under `{normalized_path}`"
                );
                eprintln!("note: selector: {}", selector.describe());
                eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
                for runnable in &runnable_targets {
                    eprintln!(
                        "note: candidate target `{}` from package `{}`",
                        project_target_display_path(
                            &runnable.member_manifest_path,
                            runnable.target.path.as_path()
                        ),
                        runnable.package_name
                    );
                }
                eprintln!(
                    "hint: refine the selector with `--package`, `--bin`, or `--target`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
                );
                return Err(1);
            }
            eprintln!(
                "error: `ql run` found multiple runnable build targets under `{normalized_path}`"
            );
            eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
            for runnable in &runnable_targets {
                eprintln!(
                    "note: candidate target `{}` from package `{}`",
                    project_target_display_path(
                        &runnable.member_manifest_path,
                        runnable.target.path.as_path()
                    ),
                    runnable.package_name
                );
            }
            eprintln!(
                "hint: rerun `ql run <source-file>` for a specific target, or `ql project targets {normalized_path}` to inspect the discovered build targets"
            );
            Err(1)
        }
    }
}

fn is_runnable_project_target(kind: BuildTargetKind) -> bool {
    matches!(kind, BuildTargetKind::Binary | BuildTargetKind::Source)
}

fn run_built_executable(executable_path: &Path, program_args: &[String]) -> Result<(), u8> {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let status = command.status().map_err(|error| {
        eprintln!(
            "error: failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        );
        1
    })?;

    match status.code() {
        Some(0) => Ok(()),
        Some(code) => std::process::exit(code),
        None => {
            eprintln!(
                "error: built executable `{}` terminated without an exit code",
                normalize_path(executable_path)
            );
            Err(1)
        }
    }
}

#[derive(Clone, Debug)]
struct TestTarget {
    display_path: String,
    kind: TestTargetKind,
}

#[derive(Clone, Debug)]
enum TestTargetKind {
    Smoke {
        source_path: PathBuf,
        working_directory: PathBuf,
        build_options: BuildOptions,
        package_manifest_path: Option<PathBuf>,
    },
    Ui {
        source_path: PathBuf,
        diagnostic_path: PathBuf,
        snapshot_path: PathBuf,
    },
}

#[derive(Clone, Debug, Default)]
struct TestCommandOptions {
    profile: BuildProfile,
    profile_overridden: bool,
    list_only: bool,
    json: bool,
    filter: Option<String>,
    package_name: Option<String>,
    target_path: Option<String>,
}

#[derive(Clone, Debug)]
enum TestFailure {
    Build {
        display_path: String,
    },
    Run {
        display_path: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    Spawn {
        display_path: String,
        error: String,
    },
    Ui {
        display_path: String,
        detail: String,
    },
}

#[derive(Clone, Debug, Default)]
struct TestExecutionReport {
    passed: usize,
    failed: usize,
    failures: Vec<TestFailure>,
}

impl TestExecutionReport {
    fn status(&self) -> &'static str {
        if self.failures.is_empty() {
            "ok"
        } else {
            "failed"
        }
    }
}

fn render_test_json_report(
    path: &Path,
    command_options: &TestCommandOptions,
    status: &'static str,
    discovered_total: usize,
    targets: &[TestTarget],
    execution_report: Option<&TestExecutionReport>,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.test.v1",
        "path": normalize_path(path),
        "requested_profile": command_options.profile.dir_name(),
        "profile_overridden": command_options.profile_overridden,
        "package_name": command_options.package_name.as_deref(),
        "filter": command_options.filter.as_deref(),
        "list_only": command_options.list_only,
        "status": status,
        "discovered_total": discovered_total,
        "selected_total": targets.len(),
        "targets": targets.iter().map(test_json_target).collect::<Vec<_>>(),
        "passed": execution_report.map_or(0, |report| report.passed),
        "failed": execution_report.map_or(0, |report| report.failed),
        "failures": execution_report
            .map(|report| report.failures.iter().map(test_json_failure).collect::<Vec<_>>())
            .unwrap_or_default(),
    }))
    .expect("test json report should serialize");
    format!("{rendered}\n")
}

fn test_json_target(target: &TestTarget) -> JsonValue {
    match &target.kind {
        TestTargetKind::Smoke { build_options, .. } => json!({
            "path": target.display_path,
            "kind": "smoke",
            "profile": build_options.profile.dir_name(),
        }),
        TestTargetKind::Ui { .. } => json!({
            "path": target.display_path,
            "kind": "ui",
            "profile": JsonValue::Null,
        }),
    }
}

fn test_json_failure(failure: &TestFailure) -> JsonValue {
    match failure {
        TestFailure::Build { display_path } => json!({
            "path": display_path,
            "kind": "build",
        }),
        TestFailure::Run {
            display_path,
            exit_code,
            stdout,
            stderr,
        } => json!({
            "path": display_path,
            "kind": "run",
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        }),
        TestFailure::Spawn {
            display_path,
            error,
        } => json!({
            "path": display_path,
            "kind": "spawn",
            "error": error,
        }),
        TestFailure::Ui {
            display_path,
            detail,
        } => json!({
            "path": display_path,
            "kind": "ui",
            "detail": detail,
        }),
    }
}

fn test_path(path: &Path, command_options: &TestCommandOptions) -> Result<(), u8> {
    let build_options = test_build_options(command_options.profile);
    let discovered_targets = discover_test_targets(path, &build_options, command_options)?;
    let discovered_total = discovered_targets.len();

    if discovered_targets.is_empty() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_report(
                    path,
                    command_options,
                    "no-tests",
                    discovered_total,
                    &[],
                    None,
                )
            );
        } else {
            report_no_tests_discovered(path, command_options.package_name.as_deref());
        }
        return Err(1);
    }

    let targets = if let Some(target_path) = command_options.target_path.as_deref() {
        let selected = select_test_targets_by_path(discovered_targets, target_path);
        if selected.is_empty() {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_report(
                        path,
                        command_options,
                        "no-match",
                        discovered_total,
                        &[],
                        None,
                    )
                );
            } else {
                report_no_matching_test_target(
                    path,
                    target_path,
                    command_options.package_name.as_deref(),
                );
            }
            return Err(1);
        }
        selected
    } else {
        discovered_targets
    };

    let targets = filter_test_targets(targets, command_options.filter.as_deref());
    if targets.is_empty() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_report(
                    path,
                    command_options,
                    "no-match",
                    discovered_total,
                    &[],
                    None,
                )
            );
        } else {
            report_no_matching_tests(
                path,
                command_options.filter.as_deref().unwrap_or_default(),
                command_options.package_name.as_deref(),
            );
        }
        return Err(1);
    }

    if command_options.list_only {
        if command_options.json {
            print!(
                "{}",
                render_test_json_report(
                    path,
                    command_options,
                    "listed",
                    discovered_total,
                    &targets,
                    None,
                )
            );
        } else {
            list_test_targets(&targets);
        }
        return Ok(());
    }

    let execution_report = execute_test_targets(
        path,
        &targets,
        command_options.json,
        &build_options,
        command_options.profile_overridden,
    )?;
    if command_options.json {
        print!(
            "{}",
            render_test_json_report(
                path,
                command_options,
                execution_report.status(),
                discovered_total,
                &targets,
                Some(&execution_report),
            )
        );
    }

    if execution_report.failures.is_empty() {
        Ok(())
    } else {
        Err(1)
    }
}

fn discover_test_targets(
    path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
) -> Result<Vec<TestTarget>, u8> {
    if should_use_project_build(path) {
        discover_project_test_targets(
            path,
            options,
            command_options.package_name.as_deref(),
            command_options.profile_overridden,
        )
    } else if let Some(request) = resolve_project_file_test_request(path) {
        let discovered = discover_project_test_targets(
            &request.manifest_path,
            options,
            command_options.package_name.as_deref(),
            command_options.profile_overridden,
        )?;
        Ok(select_test_targets_by_path(
            discovered,
            &request.display_path,
        ))
    } else {
        if let Some(package_name) = command_options.package_name.as_deref() {
            report_test_package_selector_requires_project_context(package_name);
            return Err(1);
        }
        if let Some(target_path) = command_options.target_path.as_deref() {
            report_test_target_selector_requires_project_context(target_path);
            return Err(1);
        }
        Ok(vec![direct_test_target(path, options)?])
    }
}

#[derive(Clone, Debug)]
struct ProjectFileTestRequest {
    manifest_path: PathBuf,
    display_path: String,
}

#[derive(Clone, Debug)]
struct ProjectSourceBuildTargetRequest {
    manifest_path: PathBuf,
    selector: ProjectTargetSelector,
}

fn test_build_options(profile: BuildProfile) -> BuildOptions {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = profile;
    options
}

fn direct_test_target(path: &Path, options: &BuildOptions) -> Result<TestTarget, u8> {
    let working_directory = env::current_dir().map_err(|error| {
        eprintln!("error: failed to determine the current directory for `ql test`: {error}");
        1
    })?;
    Ok(TestTarget {
        display_path: normalize_path(path),
        kind: TestTargetKind::Smoke {
            source_path: path.to_path_buf(),
            working_directory,
            build_options: options.clone(),
            package_manifest_path: None,
        },
    })
}

fn discover_project_test_targets(
    path: &Path,
    options: &BuildOptions,
    package_name: Option<&str>,
    profile_overridden: bool,
) -> Result<Vec<TestTarget>, u8> {
    let members = load_workspace_build_targets_for_command(path, "`ql test`")?;
    let members = select_workspace_test_members(path, members, package_name)?;
    let request_root = project_request_root(path);
    let mut targets = Vec::new();

    for member in members {
        let package_root = member
            .member_manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let tests_root = package_root.join("tests");
        if !tests_root.is_dir() {
            continue;
        }

        let files = collect_ql_files(&tests_root).map_err(|error| {
            eprintln!(
                "error: `ql test` failed to read `{}`: {error}",
                normalize_path(&tests_root)
            );
            1
        })?;

        for file in files {
            targets.push(project_test_target(
                &request_root,
                &member,
                &package_root,
                &file,
                options,
                profile_overridden,
            ));
        }
    }

    Ok(targets)
}

fn select_workspace_test_members(
    path: &Path,
    members: Vec<WorkspaceBuildTargets>,
    package_name: Option<&str>,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let Some(package_name) = package_name else {
        return Ok(members);
    };

    let selected = members
        .into_iter()
        .filter(|member| member.package_name == package_name)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        let normalized_path = normalize_path(path);
        eprintln!(
            "error: `ql test` package selector matched no packages under `{normalized_path}`"
        );
        eprintln!("note: selector: package `{package_name}`");
        eprintln!(
            "hint: rerun `ql project graph {normalized_path}` to inspect the discovered package/workspace members"
        );
        return Err(1);
    }

    Ok(selected)
}

fn project_test_target(
    request_root: &Path,
    member: &WorkspaceBuildTargets,
    package_root: &Path,
    file: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
) -> TestTarget {
    let display_path = display_relative_to_root(request_root, file);
    if is_project_ui_test(package_root, file) {
        return TestTarget {
            display_path,
            kind: TestTargetKind::Ui {
                source_path: file.to_path_buf(),
                diagnostic_path: package_test_command_path(package_root, file),
                snapshot_path: file.with_extension("stderr"),
            },
        };
    }

    let mut build_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    build_options.output = Some(project_test_output_path(
        &member.member_manifest_path,
        file,
        build_options.profile,
    ));
    TestTarget {
        display_path,
        kind: TestTargetKind::Smoke {
            source_path: file.to_path_buf(),
            working_directory: package_root.to_path_buf(),
            build_options,
            package_manifest_path: Some(member.member_manifest_path.clone()),
        },
    }
}

fn project_request_root(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        path.to_path_buf()
    }
}

fn resolve_project_file_test_request(path: &Path) -> Option<ProjectFileTestRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let _ = package_name(&manifest).ok()?;
    let manifest_path = manifest.manifest_path.clone();
    let package_root = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let tests_root = package_root.join("tests");
    path.strip_prefix(&tests_root).ok()?;

    Some(ProjectFileTestRequest {
        manifest_path,
        display_path: display_relative_to_root(&package_root, path),
    })
}

fn resolve_project_source_build_target_request(
    path: &Path,
) -> Option<ProjectSourceBuildTargetRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let package_name = package_name(&manifest).ok()?.to_owned();
    let display_path = project_target_display_path(&manifest.manifest_path, path);
    let targets = discover_package_build_targets(&manifest).ok()?;
    if !targets.iter().any(|target| {
        project_target_display_path(&manifest.manifest_path, target.path.as_path()) == display_path
    }) {
        return None;
    }

    Some(ProjectSourceBuildTargetRequest {
        manifest_path: manifest.manifest_path,
        selector: ProjectTargetSelector {
            package_name: Some(package_name),
            target: Some(ProjectTargetSelectorKind::DisplayPath(display_path)),
        },
    })
}

fn display_relative_to_root(root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(root) {
        normalize_path(relative)
    } else {
        normalize_path(path)
    }
}

fn project_test_output_path(
    manifest_path: &Path,
    test_path: &Path,
    profile: BuildProfile,
) -> PathBuf {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    let tests_root = package_root.join("tests");
    let relative_test = test_path.strip_prefix(&tests_root).unwrap_or(test_path);
    let default_output =
        default_output_path(package_root, test_path, profile, BuildEmit::Executable);
    let file_name = default_output
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("test"));
    let mut output_path = package_root
        .join("target")
        .join("ql")
        .join(profile.dir_name())
        .join("tests");
    if let Some(parent) = relative_test.parent()
        && !parent.as_os_str().is_empty()
    {
        output_path = output_path.join(parent);
    }
    output_path.join(file_name)
}

fn filter_test_targets(targets: Vec<TestTarget>, filter: Option<&str>) -> Vec<TestTarget> {
    let Some(filter) = filter else {
        return targets;
    };
    targets
        .into_iter()
        .filter(|target| target.display_path.contains(filter))
        .collect()
}

fn select_test_targets_by_path(targets: Vec<TestTarget>, target_path: &str) -> Vec<TestTarget> {
    targets
        .into_iter()
        .filter(|target| test_target_matches_path(target, target_path))
        .collect()
}

fn test_target_matches_path(target: &TestTarget, target_path: &str) -> bool {
    if target.display_path == target_path {
        return true;
    }

    match &target.kind {
        TestTargetKind::Smoke { source_path, .. } | TestTargetKind::Ui { source_path, .. } => {
            normalize_path(source_path) == target_path
        }
    }
}

fn list_test_targets(targets: &[TestTarget]) {
    for target in targets {
        println!("{}", target.display_path);
    }
    println!();
    println!("test listing: {} discovered", targets.len());
}

fn execute_test_targets(
    path: &Path,
    targets: &[TestTarget],
    json: bool,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<TestExecutionReport, u8> {
    let manifest_paths = test_target_manifest_paths(targets);
    let workspace_members = if !manifest_paths.is_empty() {
        prepare_reference_interfaces_for_manifests(&manifest_paths, "`ql test`", false)?;
        let workspace_members = load_workspace_build_targets_for_command(path, "`ql test`")?;
        let selected_members =
            select_project_build_plan_root_members(&workspace_members, &manifest_paths);
        prepare_project_dependency_builds(
            &workspace_members,
            &selected_members,
            "`ql test`",
            options,
            profile_overridden,
        )?;
        Some(workspace_members)
    } else {
        None
    };

    let mut report = TestExecutionReport::default();

    for target in targets {
        if !json {
            print!("test {} ... ", target.display_path);
            let _ = std::io::stdout().flush();
        }

        match &target.kind {
            TestTargetKind::Smoke {
                source_path,
                working_directory,
                build_options,
                package_manifest_path,
            } => match if let (Some(workspace_members), Some(package_manifest_path)) =
                (workspace_members.as_ref(), package_manifest_path.as_ref())
            {
                if json {
                    build_project_source_target_quiet(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
                        false,
                    )
                } else {
                    build_project_source_target_silent(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
                        false,
                    )
                }
            } else if json {
                build_single_source_target_quiet(source_path, build_options, false)
            } else {
                build_single_source_target_silent(source_path, build_options, false)
            } {
                Ok(artifact) => match execute_test_binary(&artifact.path, working_directory) {
                    Ok((exit_code, _stdout, _stderr)) if exit_code == Some(0) => {
                        if !json {
                            println!("ok");
                        }
                        report.passed += 1;
                    }
                    Ok((exit_code, stdout, stderr)) => {
                        if !json {
                            println!("FAILED");
                        }
                        report.failed += 1;
                        report.failures.push(TestFailure::Run {
                            display_path: target.display_path.clone(),
                            exit_code,
                            stdout,
                            stderr,
                        });
                    }
                    Err(error) => {
                        if !json {
                            println!("FAILED");
                        }
                        report.failed += 1;
                        report.failures.push(TestFailure::Spawn {
                            display_path: target.display_path.clone(),
                            error,
                        });
                    }
                },
                Err(_) => {
                    if !json {
                        println!("FAILED");
                    }
                    report.failed += 1;
                    report.failures.push(TestFailure::Build {
                        display_path: target.display_path.clone(),
                    });
                }
            },
            TestTargetKind::Ui {
                source_path,
                diagnostic_path,
                snapshot_path,
            } => match execute_ui_test(source_path, diagnostic_path, snapshot_path) {
                Ok(()) => {
                    if !json {
                        println!("ok");
                    }
                    report.passed += 1;
                }
                Err(detail) => {
                    if !json {
                        println!("FAILED");
                    }
                    report.failed += 1;
                    report.failures.push(TestFailure::Ui {
                        display_path: target.display_path.clone(),
                        detail,
                    });
                }
            },
        }
    }

    if json {
        return Ok(report);
    }

    if report.failures.is_empty() {
        println!();
        println!("test result: ok. {} passed; 0 failed", report.passed);
        return Ok(report);
    }

    eprintln!();
    eprintln!("failures:");
    for failure in &report.failures {
        report_test_failure(failure);
    }
    eprintln!();
    eprintln!(
        "test result: FAILED. {} passed; {} failed",
        report.passed, report.failed
    );
    Ok(report)
}

fn execute_test_binary(
    executable_path: &Path,
    working_directory: &Path,
) -> Result<(Option<i32>, String, String), String> {
    let output = Command::new(executable_path)
        .current_dir(working_directory)
        .output()
        .map_err(|error| {
            format!(
                "failed to run `{}`: {error}",
                normalize_path(executable_path)
            )
        })?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn execute_ui_test(
    source_path: &Path,
    diagnostic_path: &Path,
    snapshot_path: &Path,
) -> Result<(), String> {
    let expected = fs::read_to_string(snapshot_path).map_err(|error| {
        format!(
            "reason: failed to read expected stderr snapshot `{}`: {error}",
            normalize_path(snapshot_path)
        )
    })?;
    let source = fs::read_to_string(source_path).map_err(|error| {
        format!(
            "reason: failed to read ui test source `{}`: {error}",
            normalize_path(source_path)
        )
    })?;
    let expected = normalize_output_text(&expected);
    let actual = match analyze_source(&source) {
        Ok(()) => {
            return Err(
                "reason: ui test expected diagnostics, but the source analyzed successfully"
                    .to_owned(),
            );
        }
        Err(diagnostics) => normalize_output_text(&render_diagnostics(
            Path::new(&normalize_path(diagnostic_path)),
            &source,
            &diagnostics,
        )),
    };

    if actual != expected {
        return Err(format!(
            "reason: ui stderr snapshot mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
        ));
    }

    Ok(())
}

fn report_test_failure(failure: &TestFailure) {
    match failure {
        TestFailure::Build { display_path } => {
            eprintln!("  {display_path}");
            eprintln!("    reason: test failed to build");
        }
        TestFailure::Run {
            display_path,
            exit_code,
            stdout,
            stderr,
        } => {
            eprintln!("  {display_path}");
            match exit_code {
                Some(code) => eprintln!("    reason: test process exited with code {code}"),
                None => eprintln!("    reason: test process terminated without an exit code"),
            }
            if !stdout.trim().is_empty() {
                eprintln!("    stdout:");
                for line in stdout.lines() {
                    eprintln!("      {line}");
                }
            }
            if !stderr.trim().is_empty() {
                eprintln!("    stderr:");
                for line in stderr.lines() {
                    eprintln!("      {line}");
                }
            }
        }
        TestFailure::Spawn {
            display_path,
            error,
        } => {
            eprintln!("  {display_path}");
            eprintln!("    reason: {error}");
        }
        TestFailure::Ui {
            display_path,
            detail,
        } => {
            eprintln!("  {display_path}");
            for line in detail.lines() {
                eprintln!("    {line}");
            }
        }
    }
}

fn report_test_package_selector_requires_project_context(package_name: &str) {
    eprintln!("error: `ql test` package selectors require a package or workspace path");
    eprintln!("note: selector: package `{package_name}`");
}

fn report_test_target_selector_requires_project_context(target_path: &str) {
    eprintln!("error: `ql test` target selectors require a package or workspace path");
    eprintln!("note: selector: target `{target_path}`");
}

fn report_no_tests_discovered(path: &Path, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        eprintln!(
            "error: `ql test` found no `.ql` test files for package `{package_name}` under `{normalized_path}`"
        );
        eprintln!(
            "hint: add standalone smoke tests under `tests/**/*.ql`, rerun `ql test {normalized_path} --package {package_name} --list`, or adjust `--package`"
        );
        return;
    }
    eprintln!("error: `ql test` found no `.ql` test files under `{normalized_path}`");
    eprintln!(
        "hint: add standalone smoke tests under `tests/**/*.ql`, or rerun `ql test <file.ql>` for a single file"
    );
}

fn report_no_matching_tests(path: &Path, filter: &str, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        eprintln!(
            "error: `ql test` found no test files matching `{filter}` for package `{package_name}` under `{normalized_path}`"
        );
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--filter` / `--package`"
        );
        return;
    }
    eprintln!("error: `ql test` found no test files matching `{filter}` under `{normalized_path}`");
    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--filter`"
    );
}

fn report_no_matching_test_target(path: &Path, target_path: &str, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        eprintln!(
            "error: `ql test` found no test target `{target_path}` for package `{package_name}` under `{normalized_path}`"
        );
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--target` / `--package`"
        );
        return;
    }

    eprintln!("error: `ql test` found no test target `{target_path}` under `{normalized_path}`");
    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--target`"
    );
}

fn is_project_ui_test(package_root: &Path, source_path: &Path) -> bool {
    let Ok(relative) = source_path.strip_prefix(package_root) else {
        return false;
    };
    let mut components = relative.components().filter_map(component_name);
    matches!(components.next(), Some("tests")) && matches!(components.next(), Some("ui"))
}

fn package_test_command_path(package_root: &Path, source_path: &Path) -> PathBuf {
    source_path
        .strip_prefix(package_root)
        .unwrap_or(source_path)
        .to_path_buf()
}

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn build_project_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
) -> Result<(), u8> {
    let mut json_report =
        json.then(|| BuildJsonReport::new(path, options, profile_overridden, emit_interface));

    let members = if json {
        match load_workspace_build_targets_for_build_json(path) {
            Ok(members) => members,
            Err(failure) => return emit_build_json_failure(&mut json_report, failure),
        }
    } else {
        load_workspace_build_targets_for_command(path, "`ql build`")?
    };
    let selected_members = if json {
        match select_workspace_build_targets_for_build_json(
            path,
            &members,
            selector,
            "build targets",
        ) {
            Ok(selected) => selected,
            Err(failure) => return emit_build_json_failure(&mut json_report, failure),
        }
    } else {
        select_workspace_build_targets(path, &members, selector, "`ql build`", "build targets")?
    };
    let total_targets = selected_members
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>();
    if total_targets == 0 {
        let normalized_path = normalize_path(path);
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "project",
                    "target-discovery",
                    format!("found no discovered build targets under `{normalized_path}`"),
                    None,
                    None,
                    Some(0),
                ),
            );
        }
        eprintln!("error: `ql build` found no discovered build targets under `{normalized_path}`");
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }

    if total_targets > 1 {
        let normalized_path = normalize_path(path);
        if options.output.is_some() {
            if json {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_preflight_failure(
                        path,
                        None,
                        None,
                        None,
                        "output-conflict",
                        "output-planning",
                        "`ql build --output` only supports a single discovered build target"
                            .to_owned(),
                        None,
                        None,
                        Some(total_targets),
                    ),
                );
            }
            eprintln!("error: `ql build --output` only supports a single discovered build target");
            eprintln!("note: `{normalized_path}` resolved to {total_targets} build targets");
            return Err(1);
        }
        if options
            .c_header
            .as_ref()
            .and_then(|header| header.output.as_ref())
            .is_some()
        {
            if json {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_preflight_failure(
                        path,
                        None,
                        None,
                        None,
                        "output-conflict",
                        "output-planning",
                        "`ql build --header-output` only supports a single discovered build target"
                            .to_owned(),
                        None,
                        None,
                        Some(total_targets),
                    ),
                );
            }
            eprintln!(
                "error: `ql build --header-output` only supports a single discovered build target"
            );
            eprintln!("note: `{normalized_path}` resolved to {total_targets} build targets");
            return Err(1);
        }
    }

    if let Some(output_path) = first_colliding_project_build_output_path(
        &selected_members,
        options,
        emit_overridden,
        profile_overridden,
    ) {
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "output-conflict",
                    "output-planning",
                    format!(
                        "resolved multiple build targets to the same output path `{}`",
                        normalize_path(&output_path)
                    ),
                    None,
                    Some(normalize_path(&output_path)),
                    None,
                ),
            );
        }
        eprintln!(
            "error: `ql build` resolved multiple build targets to the same output path `{}`",
            normalize_path(&output_path)
        );
        eprintln!(
            "hint: rerun a single package/target path or rename the conflicting build target stems"
        );
        return Err(1);
    }
    if let Some(output_path) = first_colliding_project_build_header_output_path(
        &selected_members,
        options,
        emit_overridden,
        profile_overridden,
    ) {
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "output-conflict",
                    "output-planning",
                    format!(
                        "resolved multiple build targets to the same header output path `{}`",
                        normalize_path(&output_path)
                    ),
                    None,
                    Some(normalize_path(&output_path)),
                    None,
                ),
            );
        }
        eprintln!(
            "error: `ql build` resolved multiple build targets to the same header output path `{}`",
            normalize_path(&output_path)
        );
        eprintln!(
            "hint: rerun a single package/target path or rename the conflicting build target stems"
        );
        return Err(1);
    }

    let member_manifest_paths = selected_members
        .iter()
        .map(|member| member.member_manifest_path.clone())
        .collect::<Vec<_>>();
    if json {
        if let Err(failure) =
            prepare_reference_interfaces_for_manifests_quiet(&member_manifest_paths)
        {
            return emit_build_json_failure(
                &mut json_report,
                build_json_dependency_interface_prep_failure(path, &failure),
            );
        }
    } else {
        prepare_reference_interfaces_for_manifests(&member_manifest_paths, "`ql build`", true)?;
    }

    let build_plan = if json {
        match resolve_project_build_plan_members_quiet(&members, &selected_members) {
            Ok(build_plan) => build_plan,
            Err(failure) => {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_build_plan_failure(path, &failure),
                );
            }
        }
    } else {
        resolve_project_build_plan_members(&members, &selected_members, "`ql build`")?
    };

    for plan_member in &build_plan {
        if plan_member.require_targets && plan_member.member.targets.is_empty() {
            if json {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_preflight_failure(
                        path,
                        Some(&plan_member.member.member_manifest_path),
                        Some(plan_member.member.package_name.as_str()),
                        Some(true),
                        "project",
                        "build-plan",
                        format!(
                            "package `{}` has no discovered build targets",
                            plan_member.member.package_name
                        ),
                        None,
                        None,
                        Some(0),
                    ),
                );
            }
            eprintln!(
                "error: `ql build` package `{}` has no discovered build targets",
                plan_member.member.package_name
            );
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(&plan_member.member.member_manifest_path)
            );
            eprintln!(
                "hint: rerun `ql project targets {}` to inspect the discovered build targets",
                normalize_path(&plan_member.member.member_manifest_path)
            );
            return Err(1);
        }
        if plan_member.member.targets.is_empty() {
            continue;
        }

        let mut built_targets = plan_member.member.targets.iter();
        let first_target = built_targets
            .next()
            .expect("member targets emptiness checked above");
        let first_options = if plan_member.require_targets {
            project_target_build_options(
                &plan_member.member,
                first_target,
                options,
                emit_overridden,
                profile_overridden,
            )
        } else {
            project_dependency_target_build_options(
                &plan_member.member,
                first_target,
                options,
                emit_overridden,
                profile_overridden,
            )
        };
        let first_artifact = if json {
            match build_project_source_target_result(
                &build_plan,
                &plan_member.member.member_manifest_path,
                &first_target.path,
                &first_options,
                options,
                profile_overridden,
            ) {
                Ok(artifact) => artifact,
                Err(BuildTargetJsonError::Early(error)) => {
                    return emit_build_json_failure(
                        &mut json_report,
                        build_json_target_prep_failure(
                            &plan_member.member,
                            first_target,
                            plan_member.require_targets,
                            &error,
                        ),
                    );
                }
                Err(BuildTargetJsonError::Build(error)) => {
                    let mut report = json_report
                        .take()
                        .expect("json report should exist for `ql build --json`");
                    report.record_project_failure(
                        &plan_member.member,
                        first_target,
                        &error,
                        plan_member.require_targets,
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        } else {
            build_project_source_target(
                &members,
                "`ql build`",
                &plan_member.member.member_manifest_path,
                &first_target.path,
                &first_options,
                options,
                profile_overridden,
                emit_interface,
            )?
        };
        if let Some(report) = json_report.as_mut() {
            report.record_project_target(
                &plan_member.member,
                first_target,
                &first_artifact,
                plan_member.require_targets,
            );
        }
        let mut additional_artifacts = Vec::new();
        for target in built_targets {
            let target_options = if plan_member.require_targets {
                project_target_build_options(
                    &plan_member.member,
                    target,
                    options,
                    emit_overridden,
                    profile_overridden,
                )
            } else {
                project_dependency_target_build_options(
                    &plan_member.member,
                    target,
                    options,
                    emit_overridden,
                    profile_overridden,
                )
            };
            let artifact = if json {
                match build_project_source_target_result(
                    &build_plan,
                    &plan_member.member.member_manifest_path,
                    &target.path,
                    &target_options,
                    options,
                    profile_overridden,
                ) {
                    Ok(artifact) => artifact,
                    Err(BuildTargetJsonError::Early(error)) => {
                        return emit_build_json_failure(
                            &mut json_report,
                            build_json_target_prep_failure(
                                &plan_member.member,
                                target,
                                plan_member.require_targets,
                                &error,
                            ),
                        );
                    }
                    Err(BuildTargetJsonError::Build(error)) => {
                        let mut report = json_report
                            .take()
                            .expect("json report should exist for `ql build --json`");
                        report.record_project_failure(
                            &plan_member.member,
                            target,
                            &error,
                            plan_member.require_targets,
                        );
                        print!("{}", report.into_json());
                        return Err(1);
                    }
                }
            } else {
                build_project_source_target(
                    &members,
                    "`ql build`",
                    &plan_member.member.member_manifest_path,
                    &target.path,
                    &target_options,
                    options,
                    profile_overridden,
                    emit_interface,
                )?
            };
            if let Some(report) = json_report.as_mut() {
                report.record_project_target(
                    &plan_member.member,
                    target,
                    &artifact,
                    plan_member.require_targets,
                );
            }
            additional_artifacts.push(artifact.path);
        }

        if plan_member.emit_interface {
            if let Some(report) = json_report.as_mut() {
                let interface_result = match emit_built_package_interface_quiet(
                    &plan_member.member.member_manifest_path,
                    options,
                    &first_artifact.path,
                    &additional_artifacts,
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        return emit_build_json_failure(
                            &mut json_report,
                            build_json_emit_interface_failure(
                                path,
                                Some(&plan_member.member.member_manifest_path),
                                Some(plan_member.member.package_name.as_str()),
                                &error,
                            ),
                        );
                    }
                };
                report.record_interface_result(
                    Some(&plan_member.member.member_manifest_path),
                    Some(plan_member.member.package_name.as_str()),
                    true,
                    interface_result,
                );
            } else {
                emit_built_package_interface(
                    &plan_member.member.member_manifest_path,
                    options,
                    &first_artifact.path,
                    &additional_artifacts,
                )?;
            }
        }
    }

    if let Some(report) = json_report {
        print!("{}", report.into_json());
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectBuildPlanMember {
    member: WorkspaceBuildTargets,
    emit_interface: bool,
    require_targets: bool,
}

enum BuildTargetJsonError {
    Early(PrepareProjectTargetBuildError),
    Build(BuildError),
}

struct PrepareProjectTargetBuildError {
    failure_kind: PrepareProjectTargetBuildFailureKind,
}

enum PrepareProjectTargetBuildFailureKind {
    DependencyManifest {
        dependency_manifest_path: Option<PathBuf>,
        error_kind: &'static str,
        message: String,
    },
    DependencyInterface {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        interface_path: PathBuf,
        message: String,
    },
    DependencyExternConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    SourceRead {
        path: PathBuf,
        message: String,
    },
}

#[derive(Clone, Debug)]
struct DependencyExternOwner {
    package_name: String,
    manifest_path: PathBuf,
}

struct BuildPlanResolveError {
    manifest_path: Option<PathBuf>,
    owner_manifest_path: Option<PathBuf>,
    dependency_manifest_path: Option<PathBuf>,
    failure_kind: BuildPlanResolveFailureKind,
}

enum BuildPlanResolveFailureKind {
    Dependency { message: String },
    Cycle { cycle_manifests: Vec<String> },
}

fn resolve_project_build_plan_members(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
) -> Result<Vec<ProjectBuildPlanMember>, u8> {
    let workspace_members_by_manifest = workspace_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_members_by_manifest = selected_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_manifest_paths = selected_members_by_manifest
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut ordered = Vec::new();
    let mut visiting = Vec::new();
    let mut in_progress = BTreeSet::new();
    let mut visited = BTreeSet::new();

    for member in selected_members {
        collect_project_build_plan_member(
            &member.member_manifest_path,
            None,
            &workspace_members_by_manifest,
            &selected_members_by_manifest,
            &selected_manifest_paths,
            &mut visiting,
            &mut in_progress,
            &mut visited,
            &mut ordered,
            command_label,
        )?;
    }

    Ok(ordered)
}

fn resolve_project_build_plan_members_quiet(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
) -> Result<Vec<ProjectBuildPlanMember>, BuildPlanResolveError> {
    let workspace_members_by_manifest = workspace_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_members_by_manifest = selected_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_manifest_paths = selected_members_by_manifest
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut ordered = Vec::new();
    let mut visiting = Vec::new();
    let mut in_progress = BTreeSet::new();
    let mut visited = BTreeSet::new();

    for member in selected_members {
        collect_project_build_plan_member_quiet(
            &member.member_manifest_path,
            Some(&member.member_manifest_path),
            None,
            &workspace_members_by_manifest,
            &selected_members_by_manifest,
            &selected_manifest_paths,
            &mut visiting,
            &mut in_progress,
            &mut visited,
            &mut ordered,
        )?;
    }

    Ok(ordered)
}

fn select_project_build_plan_root_members(
    members: &[WorkspaceBuildTargets],
    manifest_paths: &[PathBuf],
) -> Vec<WorkspaceBuildTargets> {
    let selected_manifest_paths = manifest_paths
        .iter()
        .map(|manifest_path| normalize_path(manifest_path))
        .collect::<BTreeSet<_>>();
    members
        .iter()
        .filter(|member| {
            selected_manifest_paths.contains(&normalize_path(&member.member_manifest_path))
        })
        .cloned()
        .collect()
}

fn prepare_project_dependency_builds(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<(), u8> {
    let build_plan =
        resolve_project_build_plan_members(workspace_members, selected_members, command_label)?;
    for plan_member in &build_plan {
        if plan_member.require_targets || plan_member.member.targets.is_empty() {
            continue;
        }
        for target in &plan_member.member.targets {
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                options,
                false,
                profile_overridden,
            );
            build_project_source_target_silent(
                workspace_members,
                command_label,
                &plan_member.member.member_manifest_path,
                &target.path,
                &target_options,
                options,
                profile_overridden,
                false,
            )?;
        }
    }
    Ok(())
}

fn collect_project_build_plan_member(
    start_path: &Path,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_manifest_paths: &BTreeSet<String>,
    visiting: &mut Vec<String>,
    in_progress: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<ProjectBuildPlanMember>,
    command_label: &str,
) -> Result<(), u8> {
    let manifest = load_project_manifest(start_path).map_err(|error| {
        report_project_build_dependency_error(command_label, owner_manifest_path, &error);
        1
    })?;
    let manifest_key = normalize_path(&manifest.manifest_path);
    if visited.contains(&manifest_key) {
        return Ok(());
    }
    if in_progress.contains(&manifest_key) {
        report_project_build_dependency_cycle(command_label, visiting, &manifest_key);
        return Err(1);
    }

    in_progress.insert(manifest_key.clone());
    visiting.push(manifest_key.clone());

    let manifest_dir = manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    for reference in &manifest.references.packages {
        collect_project_build_plan_member(
            &manifest_dir.join(reference),
            Some(&manifest.manifest_path),
            workspace_members_by_manifest,
            selected_members_by_manifest,
            selected_manifest_paths,
            visiting,
            in_progress,
            visited,
            ordered,
            command_label,
        )?;
    }

    visiting.pop();
    in_progress.remove(&manifest_key);
    visited.insert(manifest_key.clone());

    let member = project_build_plan_member_targets(
        &manifest,
        &manifest_key,
        workspace_members_by_manifest,
        selected_members_by_manifest,
        command_label,
    )?;
    let selected = selected_manifest_paths.contains(&manifest_key);
    ordered.push(ProjectBuildPlanMember {
        member,
        emit_interface: selected,
        require_targets: selected,
    });

    Ok(())
}

fn collect_project_build_plan_member_quiet(
    start_path: &Path,
    dependency_manifest_path: Option<&Path>,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_manifest_paths: &BTreeSet<String>,
    visiting: &mut Vec<String>,
    in_progress: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<ProjectBuildPlanMember>,
) -> Result<(), BuildPlanResolveError> {
    let manifest = load_project_manifest(start_path).map_err(|error| {
        build_plan_dependency_failure(
            owner_manifest_path,
            dependency_manifest_path.or(Some(start_path)),
            &error,
        )
    })?;
    let manifest_key = normalize_path(&manifest.manifest_path);
    if visited.contains(&manifest_key) {
        return Ok(());
    }
    if in_progress.contains(&manifest_key) {
        return Err(build_plan_cycle_failure(visiting, &manifest_key));
    }

    in_progress.insert(manifest_key.clone());
    visiting.push(manifest_key.clone());

    let manifest_dir = manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    for reference in &manifest.references.packages {
        let reference_manifest_path = reference_manifest_path(&manifest, reference);
        collect_project_build_plan_member_quiet(
            &manifest_dir.join(reference),
            Some(&reference_manifest_path),
            Some(&manifest.manifest_path),
            workspace_members_by_manifest,
            selected_members_by_manifest,
            selected_manifest_paths,
            visiting,
            in_progress,
            visited,
            ordered,
        )?;
    }

    visiting.pop();
    in_progress.remove(&manifest_key);
    visited.insert(manifest_key.clone());

    let member = project_build_plan_member_targets_quiet(
        &manifest,
        &manifest_key,
        owner_manifest_path,
        workspace_members_by_manifest,
        selected_members_by_manifest,
    )?;
    let selected = selected_manifest_paths.contains(&manifest_key);
    ordered.push(ProjectBuildPlanMember {
        member,
        emit_interface: selected,
        require_targets: selected,
    });

    Ok(())
}

fn project_build_plan_member_targets(
    manifest: &ql_project::ProjectManifest,
    manifest_key: &str,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    command_label: &str,
) -> Result<WorkspaceBuildTargets, u8> {
    if let Some(member) = selected_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }
    if let Some(member) = workspace_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }

    let package_name = package_name(manifest).map_err(|error| {
        report_project_build_dependency_error(command_label, None, &error);
        1
    })?;
    let targets = discover_package_build_targets(manifest).map_err(|error| {
        report_project_build_dependency_error(command_label, None, &error);
        1
    })?;
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: package_name.to_owned(),
        default_profile: manifest.profile.as_ref().map(|profile| profile.default),
        targets,
    })
}

fn project_build_plan_member_targets_quiet(
    manifest: &ql_project::ProjectManifest,
    manifest_key: &str,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
) -> Result<WorkspaceBuildTargets, BuildPlanResolveError> {
    if let Some(member) = selected_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }
    if let Some(member) = workspace_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }

    let package_name = package_name(manifest).map_err(|error| {
        build_plan_dependency_failure(owner_manifest_path, Some(&manifest.manifest_path), &error)
    })?;
    let targets = discover_package_build_targets(manifest).map_err(|error| {
        build_plan_dependency_failure(owner_manifest_path, Some(&manifest.manifest_path), &error)
    })?;
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: package_name.to_owned(),
        default_profile: manifest.profile.as_ref().map(|profile| profile.default),
        targets,
    })
}

fn build_plan_dependency_failure(
    owner_manifest_path: Option<&Path>,
    dependency_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) -> BuildPlanResolveError {
    let manifest_path =
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let ql_project::ProjectError::ManifestNotFound { .. } = error {
            dependency_manifest_path.map(Path::to_path_buf)
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { .. } = error {
            dependency_manifest_path.map(Path::to_path_buf)
        } else {
            dependency_manifest_path.map(Path::to_path_buf)
        };
    BuildPlanResolveError {
        manifest_path,
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        dependency_manifest_path: dependency_manifest_path.map(Path::to_path_buf),
        failure_kind: BuildPlanResolveFailureKind::Dependency {
            message: error.to_string(),
        },
    }
}

fn build_plan_cycle_failure(
    visiting: &[String],
    repeated_manifest_path: &str,
) -> BuildPlanResolveError {
    let cycle_start = visiting
        .iter()
        .position(|manifest_path| manifest_path == repeated_manifest_path)
        .unwrap_or(0);
    let mut cycle_manifests = visiting[cycle_start..].to_vec();
    cycle_manifests.push(repeated_manifest_path.to_owned());
    BuildPlanResolveError {
        manifest_path: Some(PathBuf::from(repeated_manifest_path)),
        owner_manifest_path: None,
        dependency_manifest_path: Some(PathBuf::from(repeated_manifest_path)),
        failure_kind: BuildPlanResolveFailureKind::Cycle { cycle_manifests },
    }
}

fn report_project_build_dependency_error(
    command_label: &str,
    owner_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: {command_label} could not find `qlang.toml` for a local package dependency starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: {command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: {command_label} {error}");
    }

    if let Some(owner_manifest_path) = owner_manifest_path {
        eprintln!(
            "note: while resolving local build dependency from `{}`",
            normalize_path(owner_manifest_path)
        );
    }
}

fn target_prep_dependency_manifest_failure(
    dependency_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) -> PrepareProjectTargetBuildError {
    let dependency_manifest_path =
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else {
            dependency_manifest_path.map(Path::to_path_buf)
        };
    let error_kind = match error {
        ql_project::ProjectError::PackageSourceRootNotFound { .. } => "package-source-root",
        _ => "manifest",
    };

    PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::DependencyManifest {
            dependency_manifest_path,
            error_kind,
            message: error.to_string(),
        },
    }
}

fn report_project_build_dependency_cycle(
    command_label: &str,
    visiting: &[String],
    repeated_manifest_path: &str,
) {
    let cycle_start = visiting
        .iter()
        .position(|manifest_path| manifest_path == repeated_manifest_path)
        .unwrap_or(0);
    let mut cycle = visiting[cycle_start..].to_vec();
    cycle.push(repeated_manifest_path.to_owned());
    eprintln!("error: {command_label} local package build dependencies contain a cycle");
    eprintln!("note: cycle manifests: {}", cycle.join(" -> "));
}

#[derive(Clone, Debug, Default)]
struct PreparedProjectTargetBuild {
    source_override: Option<String>,
    additional_link_inputs: Vec<PathBuf>,
}

fn build_project_source_target(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_project_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        true,
        true,
    )
}

fn build_project_source_target_silent(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_project_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        false,
        true,
    )
}

fn build_project_source_target_result(
    build_plan: &[ProjectBuildPlanMember],
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<BuildArtifact, BuildTargetJsonError> {
    let prepared = prepare_project_target_build_quiet(
        build_plan,
        manifest_path,
        path,
        dependency_options,
        profile_overridden,
    )
    .map_err(BuildTargetJsonError::Early)?;
    build_single_source_target_with_inputs_result(
        path,
        options,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
    .map_err(BuildTargetJsonError::Build)
}

fn build_project_source_target_quiet(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_project_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        false,
        false,
    )
}

fn prepare_project_target_build_quiet(
    build_plan: &[ProjectBuildPlanMember],
    manifest_path: &Path,
    path: &Path,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<PreparedProjectTargetBuild, PrepareProjectTargetBuildError> {
    let additional_link_inputs =
        project_dependency_link_inputs(build_plan, dependency_options, profile_overridden);
    let dependency_declarations =
        render_direct_dependency_extern_declarations_quiet(manifest_path)?;

    let source_override = if dependency_declarations.is_empty() {
        None
    } else {
        let source = fs::read_to_string(path).map_err(|error| PrepareProjectTargetBuildError {
            failure_kind: PrepareProjectTargetBuildFailureKind::SourceRead {
                path: path.to_path_buf(),
                message: format!("failed to access `{}`: {error}", normalize_path(path)),
            },
        })?;
        Some(append_dependency_declarations(
            &source,
            &dependency_declarations,
        ))
    };

    Ok(PreparedProjectTargetBuild {
        source_override,
        additional_link_inputs,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_project_source_target_impl(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
    report_success: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    let prepared = prepare_project_target_build(
        workspace_members,
        command_label,
        manifest_path,
        path,
        dependency_options,
        profile_overridden,
        report_failure,
    )?;
    build_single_source_target_with_inputs_impl(
        path,
        options,
        emit_interface,
        report_success,
        report_failure,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
}

fn prepare_project_target_build(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    report_failure: bool,
) -> Result<PreparedProjectTargetBuild, u8> {
    let selected_members =
        select_project_build_plan_root_members(workspace_members, &[manifest_path.to_path_buf()]);
    let build_plan =
        resolve_project_build_plan_members(workspace_members, &selected_members, command_label)?;
    let additional_link_inputs =
        project_dependency_link_inputs(&build_plan, dependency_options, profile_overridden);
    let dependency_declarations = match render_direct_dependency_extern_declarations(
        command_label,
        manifest_path,
        report_failure,
    ) {
        Ok(declarations) => declarations,
        Err(code) => return Err(code),
    };

    let source_override = if dependency_declarations.is_empty() {
        None
    } else {
        let source = fs::read_to_string(path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: failed to access `{}`: {error}",
                    normalize_path(path)
                );
            }
            1
        })?;
        Some(append_dependency_declarations(
            &source,
            &dependency_declarations,
        ))
    };

    Ok(PreparedProjectTargetBuild {
        source_override,
        additional_link_inputs,
    })
}

fn append_dependency_declarations(source: &str, dependency_declarations: &str) -> String {
    let mut combined = source.trim_end_matches(['\r', '\n']).to_owned();
    combined.push_str("\n\n");
    combined.push_str(dependency_declarations);
    if !combined.ends_with('\n') {
        combined.push('\n');
    }
    combined
}

fn project_dependency_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    let mut outputs = Vec::new();
    for plan_member in build_plan.iter().rev() {
        if plan_member.require_targets {
            continue;
        }
        for target in &plan_member.member.targets {
            if target.kind != BuildTargetKind::Library {
                continue;
            }
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                dependency_options,
                false,
                profile_overridden,
            );
            if let Some(path) = target_options.output {
                outputs.push(path);
            }
        }
    }
    outputs
}

fn render_direct_dependency_extern_declarations(
    command_label: &str,
    manifest_path: &Path,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency extern declarations for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} found conflicting direct dependency extern imports for `{symbol}`"
                    );
                    eprintln!("note: first package: `{}`", owner.package_name);
                    eprintln!("note: conflicting package: `{dependency_package}`");
                    eprintln!(
                        "hint: keep direct dependency `extern \"c\"` names unique until package-qualified extern resolution lands"
                    );
                }
                1
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn render_direct_dependency_extern_declarations_quiet(
    manifest_path: &Path,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for reference in &manifest.references.packages {
        let reference_manifest_path = reference_manifest_path(&manifest, reference);
        let dependency_manifest =
            load_project_manifest(&manifest_dir.join(reference)).map_err(|error| {
                target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
            })?;
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
                    symbol,
                    first_package: owner.package_name,
                    first_manifest_path: owner.manifest_path,
                    conflicting_package: dependency_package.clone(),
                    conflicting_manifest_path: dependency_manifest.manifest_path.clone(),
                },
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn collect_dependency_module_extern_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    _module_source_path: &str,
    module: &Module,
    contents: &str,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function)
                if function.visibility == Visibility::Public
                    && function.abi.as_deref() == Some("c") =>
            {
                record_dependency_extern_declaration(
                    dependency_package,
                    dependency_manifest_path,
                    &function.name,
                    span_text(contents, item.span),
                    owners_by_symbol,
                    declarations,
                )?;
            }
            ItemKind::ExternBlock(extern_block)
                if extern_block.visibility == Visibility::Public && extern_block.abi == "c" =>
            {
                for function in &extern_block.functions {
                    let mut declaration = String::from("extern \"c\" pub ");
                    declaration.push_str(span_text(contents, function.span).trim());
                    record_dependency_extern_declaration(
                        dependency_package,
                        dependency_manifest_path,
                        &function.name,
                        declaration,
                        owners_by_symbol,
                        declarations,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn record_dependency_extern_declaration(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    symbol_name: &str,
    declaration: String,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    if let Some(owner) = owners_by_symbol.get(symbol_name) {
        return Err((symbol_name.to_owned(), owner.clone()));
    }
    owners_by_symbol.insert(
        symbol_name.to_owned(),
        DependencyExternOwner {
            package_name: dependency_package.to_owned(),
            manifest_path: dependency_manifest_path.to_path_buf(),
        },
    );
    declarations.push(declaration.trim().to_owned());
    Ok(())
}

fn span_text(source: &str, span: ql_span::Span) -> String {
    source
        .get(span.start..span.end)
        .unwrap_or_default()
        .to_owned()
}

fn build_single_source_target(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, true, true)
}

fn build_single_source_target_silent(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, false, true)
}

fn build_single_source_target_result(
    path: &Path,
    options: &BuildOptions,
) -> Result<BuildArtifact, BuildError> {
    build_single_source_target_with_inputs_result(path, options, None, &[])
}

fn build_single_source_target_quiet(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, false, false)
}

fn build_single_source_target_impl(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    report_success: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_with_inputs_impl(
        path,
        options,
        emit_interface,
        report_success,
        report_failure,
        None,
        &[],
    )
}

fn build_single_source_target_with_inputs_impl(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    report_success: bool,
    report_failure: bool,
    source_override: Option<&str>,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, u8> {
    match build_single_source_target_with_inputs_result(
        path,
        options,
        source_override,
        additional_link_inputs,
    ) {
        Ok(artifact) => {
            if report_success {
                println!(
                    "wrote {}: {}",
                    artifact.emit.as_str(),
                    artifact.path.display()
                );
                if let Some(header) = artifact.c_header.as_ref() {
                    println!("wrote c-header: {}", header.path.display());
                }
            }
            Ok(artifact)
        }
        Err(BuildError::InvalidInput(message)) => {
            if report_failure {
                eprintln!("error: {message}");
            }
            if emit_interface && report_failure {
                if missing_build_input_path(path, &message) {
                    report_build_input_path_failure(path, options, emit_interface);
                } else if missing_dylib_exports(&message, options) {
                    report_build_export_configuration_failure(path, options, emit_interface);
                } else if missing_build_header_import_surface(&message, options) {
                    report_build_header_import_surface_failure(path, options, emit_interface);
                } else if unsupported_build_header_emit(options) {
                    report_build_header_configuration_failure(path, options, emit_interface);
                } else if let Some(header_output_path) =
                    colliding_build_header_output_path(path, options)
                {
                    report_build_header_output_path_failure(
                        path,
                        options,
                        emit_interface,
                        &header_output_path,
                    );
                }
            }
            Err(1)
        }
        Err(BuildError::Io {
            path: io_path,
            error,
        }) => {
            if report_failure {
                eprintln!("error: failed to access `{}`: {error}", io_path.display());
            }
            if emit_interface && report_failure {
                if io_path == path {
                    report_build_input_path_failure(path, options, emit_interface);
                } else if let Some(output_path) = build_output_path(path, options) {
                    if io_targets_build_output_path(&io_path, &output_path) {
                        report_build_output_path_failure(
                            path,
                            options,
                            emit_interface,
                            &output_path,
                        );
                    } else if let Some(header_output_path) = build_header_output_path(path, options)
                    {
                        if io_targets_build_header_output_path(&io_path, &header_output_path) {
                            report_build_header_output_path_failure(
                                path,
                                options,
                                emit_interface,
                                &header_output_path,
                            );
                        }
                    }
                }
            }
            Err(1)
        }
        Err(BuildError::Toolchain {
            error,
            preserved_artifacts,
        }) => {
            if report_failure {
                eprintln!("error: {error}");
                for path in preserved_artifacts {
                    eprintln!(
                        "note: preserved intermediate artifact at `{}`",
                        path.display()
                    );
                }
            }
            if emit_interface && report_failure {
                if let Some(output_path) = build_output_path(path, options) {
                    if toolchain_targets_build_output_path(&error, &output_path) {
                        report_build_output_path_failure(
                            path,
                            options,
                            emit_interface,
                            &output_path,
                        );
                    } else {
                        report_build_toolchain_failure(path, options, emit_interface);
                    }
                } else {
                    report_build_toolchain_failure(path, options, emit_interface);
                }
            }
            Err(1)
        }
        Err(BuildError::Diagnostics {
            path: diagnostic_path,
            source,
            diagnostics,
        }) => {
            if report_failure {
                print_diagnostics(&diagnostic_path, &source, &diagnostics);
            }
            if emit_interface && report_failure {
                report_build_source_diagnostics_failure(path, options, emit_interface);
            }
            Err(1)
        }
    }
}

fn build_single_source_target_with_inputs_result(
    path: &Path,
    options: &BuildOptions,
    source_override: Option<&str>,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, BuildError> {
    match source_override {
        Some(source) => {
            build_source_with_link_inputs(path, source, options, additional_link_inputs)
        }
        None if additional_link_inputs.is_empty() => build_file(path, options),
        None => build_file_with_link_inputs(path, options, additional_link_inputs),
    }
}

fn emit_built_package_interface(
    path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<(), u8> {
    let result =
        emit_built_package_interface_impl(path, options, artifact_path, additional_artifacts)?;
    report_emit_interface_result(result);
    Ok(())
}

fn emit_built_package_interface_quiet(
    path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    let _ = (options, artifact_path, additional_artifacts);
    emit_package_interface_path_quiet(path, None, false)
}

fn emit_built_package_interface_impl(
    path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<EmitPackageInterfaceResult, u8> {
    match emit_package_interface_path(path, None, "`ql build --emit-interface`", false) {
        Ok(result) => Ok(result),
        Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
            report_build_interface_package_context_failure(path, options, true, artifact_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::SourceFailure { code, .. }) => {
            report_build_interface_source_failure(path, options, true, artifact_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(code)
        }
        Err(EmitPackageInterfaceError::Code { code, .. }) => {
            report_build_interface_failure(path, options, true, artifact_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(code)
        }
        Err(EmitPackageInterfaceError::ManifestFailure { manifest_path, .. }) => {
            report_build_interface_manifest_failure(
                path,
                options,
                true,
                artifact_path,
                &manifest_path,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path,
            source_root,
        }) => {
            report_build_interface_no_sources_failure(
                path,
                options,
                true,
                artifact_path,
                &manifest_path,
                &source_root,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::SourceRootFailure {
            manifest_path,
            source_root,
        }) => {
            report_build_interface_source_root_failure(
                path,
                options,
                true,
                artifact_path,
                &manifest_path,
                &source_root,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
            report_build_interface_output_failure(path, options, true, artifact_path, &output_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
    }
}

fn build_emit_cli_value(emit: BuildEmit) -> &'static str {
    match emit {
        BuildEmit::LlvmIr => "llvm-ir",
        BuildEmit::Assembly => "asm",
        BuildEmit::Object => "obj",
        BuildEmit::Executable => "exe",
        BuildEmit::DynamicLibrary => "dylib",
        BuildEmit::StaticLibrary => "staticlib",
    }
}

fn build_output_path(path: &Path, options: &BuildOptions) -> Option<PathBuf> {
    match &options.output {
        Some(output_path) => Some(output_path.clone()),
        None => env::current_dir().ok().map(|build_root| {
            default_output_path(&build_root, path, options.profile, options.emit)
        }),
    }
}

fn build_header_output_path(path: &Path, options: &BuildOptions) -> Option<PathBuf> {
    let header = options.c_header.as_ref()?;
    match &header.output {
        Some(output_path) => Some(output_path.clone()),
        None => {
            let artifact_path = build_output_path(path, options)?;
            Some(default_build_header_output_path(
                &artifact_path,
                path,
                header.surface,
            ))
        }
    }
}

fn project_target_build_options(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> BuildOptions {
    let mut target_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    if !emit_overridden && target.kind == BuildTargetKind::Library {
        target_options.emit = BuildEmit::StaticLibrary;
    }
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &member.member_manifest_path,
            target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }
    target_options
}

fn project_dependency_target_build_options(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> BuildOptions {
    let mut target_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    if !emit_overridden && target.kind == BuildTargetKind::Library {
        target_options.emit = BuildEmit::StaticLibrary;
    }
    target_options.output = Some(project_target_output_path(
        &member.member_manifest_path,
        target.path.as_path(),
        target_options.profile,
        target_options.emit,
    ));
    target_options.c_header = None;
    target_options
}

fn apply_manifest_default_profile(
    options: &BuildOptions,
    default_profile: Option<ManifestBuildProfile>,
    profile_overridden: bool,
) -> BuildOptions {
    let mut resolved = options.clone();
    if !profile_overridden && let Some(default_profile) = default_profile {
        resolved.profile = project_manifest_build_profile(default_profile);
    }
    resolved
}

fn project_manifest_build_profile(profile: ManifestBuildProfile) -> BuildProfile {
    match profile {
        ManifestBuildProfile::Debug => BuildProfile::Debug,
        ManifestBuildProfile::Release => BuildProfile::Release,
    }
}

fn project_target_output_path(
    manifest_path: &Path,
    target_path: &Path,
    profile: BuildProfile,
    emit: BuildEmit,
) -> PathBuf {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    let source_root = package_root.join("src");
    let relative_target = target_path
        .strip_prefix(&source_root)
        .unwrap_or(target_path);
    let default_output = default_output_path(package_root, target_path, profile, emit);
    let file_name = default_output
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifact"));
    let mut output_path = package_root
        .join("target")
        .join("ql")
        .join(profile.dir_name());
    if let Some(parent) = relative_target.parent()
        && !parent.as_os_str().is_empty()
    {
        output_path = output_path.join(parent);
    }
    output_path.join(file_name)
}

fn default_build_header_output_path(
    artifact_path: &Path,
    input_path: &Path,
    surface: CHeaderSurface,
) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface {
        CHeaderSurface::Exports => format!("{stem}.h"),
        CHeaderSurface::Imports => format!("{stem}.imports.h"),
        CHeaderSurface::Both => format!("{stem}.ffi.h"),
    };
    artifact_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(file_name)
}

fn colliding_build_header_output_path(path: &Path, options: &BuildOptions) -> Option<PathBuf> {
    let output_path = build_output_path(path, options)?;
    let header_output_path = build_header_output_path(path, options)?;
    (header_output_path == output_path).then_some(header_output_path)
}

fn first_colliding_project_build_output_path(
    members: &[WorkspaceBuildTargets],
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> Option<PathBuf> {
    let mut seen = BTreeSet::new();
    for member in members {
        for target in &member.targets {
            let target_options = project_target_build_options(
                member,
                target,
                options,
                emit_overridden,
                profile_overridden,
            );
            let output_path = build_output_path(&target.path, &target_options)?;
            if !seen.insert(output_path.clone()) {
                return Some(output_path);
            }
        }
    }
    None
}

fn first_colliding_project_build_header_output_path(
    members: &[WorkspaceBuildTargets],
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> Option<PathBuf> {
    if options.c_header.is_none() {
        return None;
    }

    let mut seen = BTreeSet::new();
    for member in members {
        for target in &member.targets {
            let target_options = project_target_build_options(
                member,
                target,
                options,
                emit_overridden,
                profile_overridden,
            );
            let output_path = build_header_output_path(&target.path, &target_options)?;
            if !seen.insert(output_path.clone()) {
                return Some(output_path);
            }
        }
    }
    None
}

fn report_remaining_build_artifacts(paths: &[PathBuf]) {
    for path in paths {
        eprintln!("note: build artifact remains at `{}`", normalize_path(path));
    }
}

fn unsupported_build_header_emit(options: &BuildOptions) -> bool {
    options.c_header.is_some()
        && !matches!(
            options.emit,
            BuildEmit::DynamicLibrary | BuildEmit::StaticLibrary
        )
}

fn missing_build_input_path(path: &Path, message: &str) -> bool {
    !path.is_file() && message.contains("is not a file")
}

fn io_targets_build_output_path(io_path: &Path, output_path: &Path) -> bool {
    io_path == output_path || output_path.starts_with(io_path)
}

fn io_targets_build_header_output_path(io_path: &Path, output_path: &Path) -> bool {
    io_path == output_path || output_path.starts_with(io_path)
}

fn toolchain_targets_build_output_path(error: &ToolchainError, output_path: &Path) -> bool {
    match error {
        ToolchainError::InvocationFailed { stderr, .. } => {
            let output_failure = stderr.to_ascii_lowercase();
            let collapsed_output_failure = output_failure
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            let mentions_output_path = collapsed_output_failure
                .contains(&normalize_path(output_path).to_ascii_lowercase())
                || collapsed_output_failure
                    .contains(&output_path.display().to_string().to_ascii_lowercase());
            mentions_output_path
                && [
                    "unable to open output file",
                    "cannot open output file",
                    "could not open output file",
                    "can't open output file",
                    "failed to open output file",
                    "unable to open file",
                    "cannot open file",
                    "could not open file",
                    "can't open file",
                    "failed to open file",
                ]
                .iter()
                .any(|message| output_failure.contains(message))
        }
        ToolchainError::NotFound { .. } => false,
    }
}

fn missing_dylib_exports(message: &str, options: &BuildOptions) -> bool {
    options.emit == BuildEmit::DynamicLibrary
        && message
            .contains("requires at least one public top-level `extern \"c\"` function definition")
}

fn missing_build_header_import_surface(message: &str, options: &BuildOptions) -> bool {
    options.c_header.is_some()
        && message.contains("does not define any imported `extern \"c\"` function declarations")
}

fn format_build_command(path: &Path, options: &BuildOptions, emit_interface: bool) -> String {
    let mut command = format!("ql build {}", normalize_path(path));
    command.push_str(&format!(" --emit {}", build_emit_cli_value(options.emit)));
    if options.profile == BuildProfile::Release {
        command.push_str(" --release");
    }
    if let Some(output) = &options.output {
        command.push_str(&format!(" --output {}", normalize_path(output)));
    }
    if let Some(header) = &options.c_header {
        if header.surface != CHeaderSurface::Exports {
            command.push_str(&format!(" --header-surface {}", header.surface.as_str()));
        } else if header.output.is_none() {
            command.push_str(" --header");
        }
        if let Some(output) = &header.output {
            command.push_str(&format!(" --header-output {}", normalize_path(output)));
        }
    }
    if emit_interface {
        command.push_str(" --emit-interface");
    }
    command
}

fn report_build_package_rerun_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    reason: &str,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` {reason}");
    }
}

fn report_build_source_diagnostics_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package sources",
    );
}

fn report_build_toolchain_failure(path: &Path, options: &BuildOptions, emit_interface: bool) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the build toolchain",
    );
}

fn report_build_export_configuration_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the dylib export surface",
    );
}

fn report_build_header_import_surface_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the build header import surface",
    );
}

fn report_build_input_path_failure(path: &Path, options: &BuildOptions, emit_interface: bool) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!("note: failing build input path: {}", normalize_path(path));
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build input path");
    }
}

fn report_build_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing build output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build output path");
    }
}

fn report_build_header_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing build header output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build header output path");
    }
}

fn report_build_header_configuration_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build header configuration");
    }
}

fn report_build_interface_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package interface error",
    );
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_build_interface_package_context_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    eprintln!(
        "note: `ql build --emit-interface` only emits package interfaces for sources inside a package"
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this source");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_build_interface_source_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package sources",
    );
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_build_interface_manifest_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
) {
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_build_interface_source_root_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
    source_root: &Path,
) {
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_build_interface_no_sources_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
    source_root: &Path,
) {
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

fn report_project_emit_interface_package_context_failure(
    path: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) {
    let command = if check_only {
        "ql project emit-interface --check"
    } else {
        "ql project emit-interface"
    };
    let action = if check_only { "checks" } else { "emits" };
    eprintln!(
        "note: `{command}` only {action} package interfaces for packages or workspace members discoverable from `qlang.toml`"
    );
    let normalized_path = normalize_path(path);
    let rerun_command = format_project_emit_interface_command(
        Some(normalized_path.as_str()),
        requested_output_path,
        changed_only,
        check_only,
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

fn report_package_interface_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package interface error",
        rerun_command
    );
}

fn report_package_interface_source_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package sources",
        rerun_command
    );
}

fn report_package_interface_manifest_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package manifest",
        rerun_command
    );
}

fn report_package_interface_source_root_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    source_root: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package source root",
        rerun_command
    );
}

fn report_package_interface_no_sources_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    source_root: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after adding package source files",
        rerun_command
    );
}

fn report_package_interface_output_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    output_path: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing interface output path: {}",
        normalize_path(output_path)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the interface output path",
        rerun_command
    );
}

fn report_build_interface_output_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing interface output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the interface output path");
    }
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
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

fn project_init_path(path: &Path, workspace: bool, package_name: Option<&str>) -> Result<(), u8> {
    let target_root = path.to_path_buf();
    let package_name = match project_init_package_name(&target_root, workspace, package_name) {
        Ok(package_name) => package_name,
        Err(message) => {
            eprintln!("error: `ql project init` {message}");
            return Err(1);
        }
    };

    let created_paths = if workspace {
        init_workspace_project(&target_root, &package_name)
    } else {
        init_package_project(&target_root, &package_name)
    }
    .map_err(|message| {
        eprintln!("error: `ql project init` {message}");
        1
    })?;

    for path in created_paths {
        println!("created: {}", normalize_path(&path));
    }

    Ok(())
}

fn project_init_package_name(
    target_root: &Path,
    workspace: bool,
    package_name: Option<&str>,
) -> Result<String, String> {
    let package_name = match package_name {
        Some(package_name) => package_name.to_owned(),
        None if workspace => "app".to_owned(),
        None => target_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                format!(
                    "could not derive a package name from `{}`; rerun with `--name <package>`",
                    normalize_path(target_root)
                )
            })?,
    };

    validate_project_init_package_name(&package_name)?;
    Ok(package_name)
}

fn validate_project_init_package_name(package_name: &str) -> Result<(), String> {
    if package_name.trim().is_empty() {
        return Err("requires a non-empty package name".to_owned());
    }
    if package_name == "." || package_name == ".." {
        return Err(format!(
            "does not accept reserved package name `{package_name}`"
        ));
    }
    if package_name.contains(['/', '\\']) {
        return Err(format!(
            "does not accept package name `{package_name}` because it contains a path separator"
        ));
    }
    Ok(())
}

fn init_package_project(target_root: &Path, package_name: &str) -> Result<Vec<PathBuf>, String> {
    ensure_project_init_target_root(target_root)?;

    let manifest_path = target_root.join("qlang.toml");
    let source_path = target_root.join("src").join("lib.ql");
    let main_path = target_root.join("src").join("main.ql");
    let test_path = target_root.join("tests").join("smoke.ql");
    let manifest = render_package_manifest(package_name);

    write_new_project_file(&manifest_path, &manifest)?;
    write_new_project_file(&source_path, default_package_source())?;
    write_new_project_file(&main_path, default_package_main_source())?;
    write_new_project_file(&test_path, default_package_test_source())?;

    Ok(vec![manifest_path, source_path, main_path, test_path])
}

fn init_workspace_project(target_root: &Path, package_name: &str) -> Result<Vec<PathBuf>, String> {
    ensure_project_init_target_root(target_root)?;

    let workspace_manifest_path = target_root.join("qlang.toml");
    let member_dir = target_root.join("packages").join(package_name);
    let member_manifest_path = member_dir.join("qlang.toml");
    let member_source_path = member_dir.join("src").join("lib.ql");
    let member_main_path = member_dir.join("src").join("main.ql");
    let member_test_path = member_dir.join("tests").join("smoke.ql");
    let workspace_manifest = render_workspace_manifest(package_name);
    let member_manifest = render_package_manifest(package_name);

    write_new_project_file(&workspace_manifest_path, &workspace_manifest)?;
    write_new_project_file(&member_manifest_path, &member_manifest)?;
    write_new_project_file(&member_source_path, default_package_source())?;
    write_new_project_file(&member_main_path, default_package_main_source())?;
    write_new_project_file(&member_test_path, default_package_test_source())?;

    Ok(vec![
        workspace_manifest_path,
        member_manifest_path,
        member_source_path,
        member_main_path,
        member_test_path,
    ])
}

fn ensure_project_init_target_root(target_root: &Path) -> Result<(), String> {
    if target_root.exists() && !target_root.is_dir() {
        return Err(format!(
            "target path `{}` already exists and is not a directory",
            normalize_path(target_root)
        ));
    }
    Ok(())
}

fn write_new_project_file(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "would overwrite existing path `{}`",
            normalize_path(path)
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create directory `{}`: {error}",
                normalize_path(parent)
            )
        })?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write `{}`: {error}", normalize_path(path)))
}

fn render_package_manifest(package_name: &str) -> String {
    format!("[package]\nname = \"{package_name}\"\n")
}

fn render_workspace_manifest(package_name: &str) -> String {
    format!("[workspace]\nmembers = [\"packages/{package_name}\"]\n")
}

fn default_package_source() -> &'static str {
    "pub fn run() -> Int {\n    return 0\n}\n"
}

fn default_package_main_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

fn default_package_test_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

fn project_graph_path(path: &Path, json: bool) -> Result<(), u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            eprintln!(
                "error: `ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            );
            report_project_graph_package_context_failure(path);
        } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: `ql project graph` manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
            report_project_graph_manifest_failure(manifest_path);
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: `ql project graph` {error}");
            report_project_graph_manifest_failure(manifest_path);
        } else {
            eprintln!("error: {error}");
        }
        1
    })?;
    let rendered = if json {
        render_project_graph_resolved_json(&manifest)
    } else {
        render_project_graph_resolved(&manifest)
    }
    .map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    print!("{rendered}");
    Ok(())
}

fn project_lock_path(path: &Path, check_only: bool) -> Result<(), u8> {
    let command_label = if check_only {
        "`ql project lock --check`"
    } else {
        "`ql project lock`"
    };

    let manifest = load_project_manifest(path).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            eprintln!(
                "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            );
            report_project_lock_package_context_failure(path, check_only);
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(&error)
        {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error)
        {
            eprintln!("error: {command_label} {error}");
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;

    let lockfile_path = project_lockfile_path(&manifest);
    let rendered = render_project_lockfile(&manifest).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error {
            eprintln!(
                "error: {command_label} package source directory `{}` does not exist",
                normalize_path(path)
            );
            eprintln!(
                "hint: rerun `{}` after fixing the package source root",
                format_project_lock_command(&manifest.manifest_path, check_only)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {command_label} {error}");
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else {
            eprintln!("error: {command_label} {error}");
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(&manifest.manifest_path)
            );
        }
        1
    })?;

    if check_only {
        return check_project_lockfile(&manifest, &lockfile_path, &rendered);
    }

    fs::write(&lockfile_path, rendered).map_err(|error| {
        eprintln!(
            "error: {command_label} failed to write lockfile `{}`: {error}",
            normalize_path(&lockfile_path)
        );
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "hint: rerun `ql project lock {}` after fixing the lockfile output path",
            normalize_path(&manifest.manifest_path)
        );
        1
    })?;

    println!("wrote lockfile: {}", normalize_path(&lockfile_path));
    Ok(())
}

fn check_project_lockfile(
    manifest: &ql_project::ProjectManifest,
    lockfile_path: &Path,
    expected: &str,
) -> Result<(), u8> {
    let normalized_lockfile_path = normalize_path(lockfile_path);
    let rerun_command = format!(
        "ql project lock {}",
        normalize_path(&manifest.manifest_path)
    );

    match fs::read_to_string(lockfile_path) {
        Ok(actual) => {
            if normalize_line_endings(&actual) == normalize_line_endings(expected) {
                return Ok(());
            }
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is stale"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is missing"
            );
        }
        Err(error) => {
            eprintln!(
                "error: `ql project lock --check` failed to read lockfile `{normalized_lockfile_path}`: {error}"
            );
        }
    }

    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(&manifest.manifest_path)
    );
    eprintln!("hint: rerun `{rerun_command}` to regenerate `qlang.lock`");
    Err(1)
}

fn load_workspace_build_targets_for_command(
    path: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(path).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            eprintln!(
                "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            );
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(&error)
        {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error)
        {
            eprintln!("error: {command_label} {error}");
            eprintln!("note: failing package manifest: {}", normalize_path(manifest_path));
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;

    discover_workspace_build_targets(&manifest).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error {
            eprintln!(
                "error: {command_label} package source directory `{}` does not exist",
                normalize_path(path)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {command_label} {error}");
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(manifest_path)
            );
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })
}

fn project_targets_path(path: &Path, json: bool) -> Result<(), u8> {
    let members = load_workspace_build_targets_for_command(path, "`ql project targets`")?;

    if json {
        print!("{}", render_project_targets_json(&members));
        return Ok(());
    }

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            println!();
        }
        print_project_target_member(
            member.member_manifest_path.as_path(),
            &member.package_name,
            &member.targets,
        );
    }

    Ok(())
}

fn print_project_target_member(manifest_path: &Path, package_name: &str, targets: &[BuildTarget]) {
    println!("manifest: {}", normalize_path(manifest_path));
    println!("package: {package_name}");

    if targets.is_empty() {
        println!("targets: (none found)");
        return;
    }

    println!("targets:");
    for target in targets {
        println!(
            "  - {}: {}",
            target.kind.as_str(),
            project_target_display_path(manifest_path, target.path.as_path())
        );
    }
}

fn project_target_display_path(manifest_path: &Path, target_path: &Path) -> String {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    if let Ok(relative) = target_path.strip_prefix(package_root) {
        normalize_path(relative)
    } else {
        normalize_path(target_path)
    }
}

fn render_project_targets_json(members: &[WorkspaceBuildTargets]) -> String {
    let mut rendered = String::new();
    rendered.push_str("{\n");
    rendered.push_str("  \"schema\": \"ql.project.targets.v1\",\n");
    rendered.push_str("  \"members\": [");

    if members.is_empty() {
        rendered.push_str("]\n}\n");
        return rendered;
    }

    rendered.push('\n');
    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            rendered.push_str(",\n");
        }
        rendered.push_str("    {\n");
        rendered.push_str("      \"manifest_path\": ");
        rendered.push_str(&json_string(&normalize_path(
            member.member_manifest_path.as_path(),
        )));
        rendered.push_str(",\n");
        rendered.push_str("      \"package_name\": ");
        rendered.push_str(&json_string(&member.package_name));
        rendered.push_str(",\n");
        rendered.push_str("      \"targets\": [");

        if member.targets.is_empty() {
            rendered.push_str("]\n");
        } else {
            rendered.push('\n');
            for (target_index, target) in member.targets.iter().enumerate() {
                if target_index > 0 {
                    rendered.push_str(",\n");
                }
                rendered.push_str("        {\n");
                rendered.push_str("          \"kind\": ");
                rendered.push_str(&json_string(target.kind.as_str()));
                rendered.push_str(",\n");
                rendered.push_str("          \"path\": ");
                rendered.push_str(&json_string(&project_target_display_path(
                    member.member_manifest_path.as_path(),
                    target.path.as_path(),
                )));
                rendered.push_str("\n        }");
            }
            rendered.push_str("\n      ]\n");
        }

        rendered.push_str("    }");
    }

    rendered.push_str("\n  ]\n}\n");
    rendered
}

fn project_emit_interface_path(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> Result<(), u8> {
    if check_only && output.is_some() {
        eprintln!("error: `ql project emit-interface --check` does not support `--output`");
        return Err(1);
    }

    let emit_command_label =
        format_project_emit_interface_command_label(output, changed_only, false);
    let check_command_label = format_project_emit_interface_command_label(None, changed_only, true);
    let manifest = load_project_manifest(path).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            let command_label = if check_only {
                check_command_label.as_str()
            } else {
                emit_command_label.as_str()
            };
            eprintln!(
                "error: {} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                command_label,
                normalize_path(start)
            );
            report_project_emit_interface_package_context_failure(
                path,
                output,
                changed_only,
                check_only,
            );
            return 1;
        }
        if check_only {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    check_command_label,
                    normalize_path(manifest_path)
                );
                report_package_interface_check_manifest_failure(manifest_path, changed_only);
                return 1;
            }
            if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
                eprintln!("error: {check_command_label} {error}");
                report_package_interface_check_manifest_failure(manifest_path, changed_only);
                return 1;
            }
        } else {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    emit_command_label,
                    normalize_path(manifest_path)
                );
                report_package_interface_manifest_failure(
                    manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return 1;
            }
            if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
                eprintln!("error: {emit_command_label} {error}");
                report_package_interface_manifest_failure(
                    manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return 1;
            }
        }
        eprintln!("error: {error}");
        1
    })?;

    if manifest.package.is_some() {
        if check_only {
            let result = match check_package_interface_artifact(
                &manifest,
                check_command_label.as_str(),
                changed_only,
            ) {
                Ok(result) => result,
                Err(code) => {
                    report_package_interface_failure(
                        &manifest.manifest_path,
                        None,
                        None,
                        changed_only,
                        None,
                    );
                    return Err(code);
                }
            };
            return report_package_interface_check(
                result,
                None,
                check_command_label.as_str(),
                changed_only,
            );
        }
        match emit_package_interface_path(path, output, emit_command_label.as_str(), changed_only) {
            Ok(result) => report_emit_interface_result(result),
            Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
                report_package_interface_failure(
                    &manifest.manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return Err(1);
            }
            Err(EmitPackageInterfaceError::SourceFailure { code, .. }) => {
                report_package_interface_source_failure(
                    &manifest.manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return Err(code);
            }
            Err(EmitPackageInterfaceError::Code { code, .. }) => {
                report_package_interface_failure(
                    &manifest.manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return Err(code);
            }
            Err(EmitPackageInterfaceError::ManifestFailure { .. }) => {
                report_package_interface_manifest_failure(
                    &manifest.manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return Err(1);
            }
            Err(EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. }) => {
                report_package_interface_no_sources_failure(
                    &manifest.manifest_path,
                    None,
                    &source_root,
                    output,
                    changed_only,
                    None,
                );
                return Err(1);
            }
            Err(EmitPackageInterfaceError::SourceRootFailure { source_root, .. }) => {
                report_package_interface_source_root_failure(
                    &manifest.manifest_path,
                    None,
                    &source_root,
                    output,
                    changed_only,
                    None,
                );
                return Err(1);
            }
            Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
                report_package_interface_output_failure(
                    &manifest.manifest_path,
                    None,
                    &output_path,
                    output,
                    changed_only,
                    None,
                );
                return Err(1);
            }
        }
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
    let mut failing_member_count = 0usize;
    let mut emission_failure_count = 0usize;
    let mut first_failing_member_manifest = None;
    for member in &workspace.members {
        let member_manifest_path = workspace_member_manifest_path(&manifest_dir.join(member));
        if check_only {
            let member_manifest = match load_project_manifest(&manifest_dir.join(member)) {
                Ok(manifest) => manifest,
                Err(error) => {
                    if let Some(manifest_path) =
                        package_missing_name_manifest_path_from_project_error(&error)
                    {
                        eprintln!(
                            "error: {} manifest `{}` does not declare `[package].name`",
                            check_command_label,
                            normalize_path(manifest_path)
                        );
                        report_workspace_member_package_interface_check_manifest_failure(
                            manifest_path,
                            changed_only,
                        );
                    } else if let Some(manifest_path) =
                        package_check_manifest_path_from_project_error(&error)
                    {
                        eprintln!("error: {check_command_label} {error}");
                        report_workspace_member_package_interface_check_manifest_failure(
                            manifest_path,
                            changed_only,
                        );
                    } else {
                        eprintln!("error: {error}");
                        let rerun_command = format_workspace_member_emit_rerun_command(
                            &normalize_path(&member_manifest_path),
                            changed_only,
                            check_only,
                        );
                        let rerun_hint = format!(
                            "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                        );
                        report_workspace_member_failure(
                            &member_manifest_path,
                            Some(rerun_hint.as_str()),
                        );
                    }
                    failing_member_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest_path.clone(),
                    );
                    continue;
                }
            };
            if let Err(error) = package_name(&member_manifest) {
                eprintln!("error: {check_command_label} {error}");
                report_workspace_member_package_interface_check_manifest_failure(
                    &member_manifest.manifest_path,
                    changed_only,
                );
                failing_member_count += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
                continue;
            }
            let result = match check_package_interface_artifact(
                &member_manifest,
                check_command_label.as_str(),
                changed_only,
            ) {
                Ok(result) => result,
                Err(_) => {
                    let rerun_command = format_workspace_member_emit_rerun_command(
                        &normalize_path(&member_manifest.manifest_path),
                        changed_only,
                        check_only,
                    );
                    let rerun_hint = format!(
                        "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                    );
                    report_workspace_member_failure(
                        &member_manifest.manifest_path,
                        Some(rerun_hint.as_str()),
                    );
                    failing_member_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                    continue;
                }
            };
            if report_package_interface_check(
                result,
                Some(&member_manifest.manifest_path),
                check_command_label.as_str(),
                changed_only,
            )
            .is_err()
            {
                failing_member_count += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
            }
        } else {
            let member_manifest = match load_project_manifest(&manifest_dir.join(member)) {
                Ok(manifest) => manifest,
                Err(error) => {
                    if let Some(manifest_path) =
                        package_missing_name_manifest_path_from_project_error(&error)
                    {
                        eprintln!(
                            "error: {} manifest `{}` does not declare `[package].name`",
                            emit_command_label,
                            normalize_path(manifest_path)
                        );
                        report_package_interface_manifest_failure(
                            manifest_path,
                            Some(manifest_path),
                            None,
                            changed_only,
                            None,
                        );
                    } else if let Some(manifest_path) =
                        package_check_manifest_path_from_project_error(&error)
                    {
                        eprintln!("error: {emit_command_label} {error}");
                        report_package_interface_manifest_failure(
                            manifest_path,
                            Some(manifest_path),
                            None,
                            changed_only,
                            None,
                        );
                    } else {
                        eprintln!("error: {error}");
                        let rerun_command = format_workspace_member_emit_rerun_command(
                            &normalize_path(&member_manifest_path),
                            changed_only,
                            check_only,
                        );
                        let rerun_hint = format!(
                            "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                        );
                        report_workspace_member_failure(
                            &member_manifest_path,
                            Some(rerun_hint.as_str()),
                        );
                    }
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest_path.clone(),
                    );
                    continue;
                }
            };
            match emit_package_interface_path(
                &member_manifest.manifest_path,
                None,
                emit_command_label.as_str(),
                changed_only,
            ) {
                Ok(result) => report_emit_interface_result(result),
                Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
                    report_package_interface_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceFailure { .. }) => {
                    report_package_interface_source_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::Code { .. }) => {
                    report_package_interface_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. }) => {
                    report_package_interface_no_sources_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        &source_root,
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::ManifestFailure { .. }) => {
                    report_package_interface_manifest_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceRootFailure { source_root, .. }) => {
                    report_package_interface_source_root_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        &source_root,
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
                    report_package_interface_output_failure(
                        &member_manifest.manifest_path,
                        Some(&member_manifest.manifest_path),
                        &output_path,
                        None,
                        changed_only,
                        None,
                    );
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
            }
        }
    }

    if check_only && failing_member_count > 0 {
        eprintln!("error: {check_command_label} found {failing_member_count} failing member(s)");
        if failing_member_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    if !check_only && emission_failure_count > 0 {
        eprintln!("error: {emit_command_label} found {emission_failure_count} failing member(s)");
        if emission_failure_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
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
    UpToDate(PathBuf),
    Invalid {
        path: PathBuf,
        status: InterfaceArtifactStatus,
        manifest_path: PathBuf,
        detail: Option<String>,
        stale_reasons: Vec<InterfaceArtifactStaleReason>,
    },
}

enum EmitPackageInterfaceError {
    Code {
        code: u8,
        message: Option<String>,
    },
    SourceFailure {
        code: u8,
        failure_count: usize,
        first_failing_source: Option<PathBuf>,
    },
    ManifestNotFound {
        start: PathBuf,
    },
    ManifestFailure {
        manifest_path: PathBuf,
        message: String,
    },
    NoSourceFilesFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    SourceRootFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    OutputPathFailure {
        manifest_path: Option<PathBuf>,
        output_path: PathBuf,
        message: String,
    },
}

#[derive(Default)]
struct ReferenceInterfaceSyncResult {
    written: Vec<PathBuf>,
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
}

#[derive(Default)]
struct ReferenceInterfaceCheckResult {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
}

struct ReferenceInterfacePrepError {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
    first_failure: ReferenceInterfacePrepFailure,
}

struct ReferenceInterfacePrepFailure {
    owner_manifest_path: Option<PathBuf>,
    reference: Option<String>,
    reference_manifest_path: PathBuf,
    manifest_path: Option<PathBuf>,
    failure_kind: ReferenceInterfacePrepFailureKind,
}

enum ReferenceInterfacePrepFailureKind {
    Project {
        error_kind: &'static str,
        message: String,
        source_root: Option<PathBuf>,
    },
    InterfaceEmit(EmitPackageInterfaceError),
}

#[derive(Default)]
struct ReferenceInterfaceSyncQuietResult {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
    first_failure: Option<ReferenceInterfacePrepFailure>,
}

fn emit_package_interface_path(
    path: &Path,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    emit_package_interface_path_impl(path, output, command_label, changed_only, true)
}

fn emit_package_interface_path_quiet(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    emit_package_interface_path_impl(
        path,
        output,
        "`ql build --emit-interface`",
        changed_only,
        false,
    )
}

fn emit_package_interface_path_impl(
    path: &Path,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
    report_failure: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    let manifest = load_project_manifest(path).map_err(|error| match error {
        ql_project::ProjectError::ManifestNotFound { start } => {
            if report_failure {
                eprintln!(
                    "error: {command_label} requires a package manifest; could not find `qlang.toml` starting from `{}`",
                    normalize_path(&start)
                );
            }
            EmitPackageInterfaceError::ManifestNotFound { start }
        }
        error => {
            if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error)
            {
                if report_failure {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        command_label,
                        normalize_path(manifest_path)
                    );
                }
                EmitPackageInterfaceError::ManifestFailure {
                    manifest_path: manifest_path.to_path_buf(),
                    message: format!(
                        "manifest `{}` does not declare `[package].name`",
                        normalize_path(manifest_path)
                    ),
                }
            } else if let Some(manifest_path) =
                package_check_manifest_path_from_project_error(&error)
            {
                if report_failure {
                    eprintln!("error: {command_label} {error}");
                }
                EmitPackageInterfaceError::ManifestFailure {
                    manifest_path: manifest_path.to_path_buf(),
                    message: error.to_string(),
                }
            } else {
                if report_failure {
                    eprintln!("error: {error}");
                }
                EmitPackageInterfaceError::Code {
                    code: 1,
                    message: Some(error.to_string()),
                }
            }
        }
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        EmitPackageInterfaceError::ManifestFailure {
            manifest_path: manifest.manifest_path.clone(),
            message: error.to_string(),
        }
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
    let source_root =
        package_source_root(&manifest).expect("package interface emission requires a package");
    let files = collect_package_sources(&manifest).map_err(|error| match error {
        ql_project::ProjectError::PackageSourceRootNotFound { path } => {
            if report_failure {
                eprintln!(
                    "error: {command_label} package source directory `{}` does not exist",
                    normalize_path(&path)
                );
            }
            EmitPackageInterfaceError::SourceRootFailure {
                manifest_path: manifest.manifest_path.clone(),
                source_root: path,
            }
        }
        error => {
            if report_failure {
                eprintln!("error: {error}");
            }
            EmitPackageInterfaceError::Code {
                code: 1,
                message: Some(error.to_string()),
            }
        }
    })?;
    if files.is_empty() {
        if report_failure {
            eprintln!(
                "error: {command_label} no `.ql` files found under `{}`",
                normalize_path(&source_root)
            );
        }
        return Err(EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path: manifest.manifest_path.clone(),
            source_root,
        });
    }

    let mut rendered_modules = Vec::new();
    let mut failing_source_count = 0usize;
    let mut first_failing_source = None;
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            if report_failure {
                eprintln!("error: failed to read `{}`: {error}", file.display());
            }
            error
        });
        let source = match source {
            Ok(source) => source,
            Err(_) => {
                failing_source_count += 1;
                record_first_failing_path(&mut first_failing_source, &file);
                continue;
            }
        };
        let analysis = match analyze_semantics(&source) {
            Ok(analysis) => analysis,
            Err(diagnostics) => {
                if report_failure {
                    print_diagnostics(&file, &source, &diagnostics);
                }
                failing_source_count += 1;
                record_first_failing_path(&mut first_failing_source, &file);
                continue;
            }
        };
        if analysis.has_errors() {
            if report_failure {
                print_diagnostics(&file, &source, analysis.diagnostics());
            }
            failing_source_count += 1;
            record_first_failing_path(&mut first_failing_source, &file);
            continue;
        }
        if let Some(rendered) = render_module_interface(analysis.ast()) {
            let relative = file.strip_prefix(manifest_dir).unwrap_or(&file);
            rendered_modules.push((normalize_path(relative), rendered));
        }
    }

    if failing_source_count > 0 {
        if report_failure {
            eprintln!("error: {command_label} found {failing_source_count} failing source file(s)");
            if failing_source_count > 1 {
                if let Some(path) = &first_failing_source {
                    eprintln!("note: first failing source file: {}", normalize_path(path));
                }
            }
        }
        return Err(EmitPackageInterfaceError::SourceFailure {
            code: 1,
            failure_count: failing_source_count,
            first_failing_source,
        });
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: failed to create interface output directory `{}`: {error}",
                    parent.display()
                );
            }
            EmitPackageInterfaceError::OutputPathFailure {
                manifest_path: Some(manifest.manifest_path.clone()),
                output_path: output_path.clone(),
                message: format!(
                    "failed to create interface output directory `{}`: {error}",
                    normalize_path(parent)
                ),
            }
        })?;
    }

    let rendered = render_interface_artifact(package_name, &rendered_modules);
    fs::write(&output_path, rendered).map_err(|error| {
        if report_failure {
            eprintln!(
                "error: failed to write interface `{}`: {error}",
                output_path.display()
            );
        }
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: Some(manifest.manifest_path.clone()),
            output_path: output_path.clone(),
            message: format!(
                "failed to write interface `{}`: {error}",
                normalize_path(&output_path)
            ),
        }
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

fn report_interface_artifact_failure(
    error_line: &str,
    detail: Option<&str>,
    stale_reasons: &[InterfaceArtifactStaleReason],
    notes: &[&str],
    hint_line: &str,
) {
    eprintln!("{error_line}");
    if let Some(detail) = detail {
        eprintln!("detail: {detail}");
    }
    report_interface_stale_reasons(stale_reasons);
    for note in notes {
        eprintln!("{note}");
    }
    eprintln!("{hint_line}");
}

fn format_emit_interface_rerun_command(
    manifest_path: &str,
    requested_output_path: Option<&Path>,
    changed_only: bool,
) -> String {
    format_project_emit_interface_command(
        Some(manifest_path),
        requested_output_path,
        changed_only,
        false,
    )
}

fn format_project_emit_interface_command(
    manifest_path: Option<&str>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> String {
    let mut command = String::from("ql project emit-interface");
    if let Some(manifest_path) = manifest_path {
        command.push(' ');
        command.push_str(manifest_path);
    }
    if changed_only {
        command.push_str(" --changed-only");
    }
    if check_only {
        command.push_str(" --check");
    }
    if let Some(output_path) = requested_output_path {
        command.push_str(&format!(" --output {}", normalize_path(output_path)));
    }
    command
}

fn format_project_emit_interface_command_label(
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> String {
    format!(
        "`{}`",
        format_project_emit_interface_command(
            None,
            requested_output_path,
            changed_only,
            check_only,
        )
    )
}

fn format_emit_interface_regenerate_command(manifest_path: &str, changed_only: bool) -> String {
    if changed_only {
        format!("ql project emit-interface {manifest_path} --changed-only")
    } else {
        format!("ql project emit-interface {manifest_path}")
    }
}

fn format_workspace_member_emit_rerun_command(
    manifest_path: &str,
    changed_only: bool,
    check_only: bool,
) -> String {
    format_project_emit_interface_command(Some(manifest_path), None, changed_only, check_only)
}

fn format_check_command(sync_interfaces: bool, manifest_path: Option<&str>) -> String {
    let mut command = String::from("ql check");
    if sync_interfaces {
        command.push_str(" --sync-interfaces");
    }
    if let Some(manifest_path) = manifest_path {
        command.push(' ');
        command.push_str(manifest_path);
    }
    command
}

fn format_check_command_label(sync_interfaces: bool) -> String {
    format!("`{}`", format_check_command(sync_interfaces, None))
}

fn format_workspace_member_check_rerun_command(
    manifest_path: &str,
    sync_interfaces: bool,
) -> String {
    format_check_command(sync_interfaces, Some(manifest_path))
}

fn check_package_interface_artifact(
    manifest: &ql_project::ProjectManifest,
    command_label: &str,
    changed_only: bool,
) -> Result<CheckPackageInterfaceResult, u8> {
    let output_path = default_interface_path(manifest).map_err(|error| {
        eprintln!("error: {command_label} {error}");
        1
    })?;
    let status = interface_artifact_status(manifest, &output_path);
    if status == InterfaceArtifactStatus::Valid {
        return Ok(if changed_only {
            CheckPackageInterfaceResult::UpToDate(output_path)
        } else {
            CheckPackageInterfaceResult::Ok(output_path)
        });
    }
    if status != InterfaceArtifactStatus::Valid {
        let detail = interface_artifact_status_detail(&output_path, status);
        let stale_reasons = if status == InterfaceArtifactStatus::Stale {
            interface_artifact_stale_reasons(manifest, &output_path)
        } else {
            Vec::new()
        };
        return Ok(CheckPackageInterfaceResult::Invalid {
            path: output_path,
            status,
            manifest_path: manifest.manifest_path.clone(),
            detail,
            stale_reasons,
        });
    }
    Ok(CheckPackageInterfaceResult::Ok(output_path))
}

fn report_package_interface_check(
    result: CheckPackageInterfaceResult,
    workspace_member_manifest_path: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<(), u8> {
    match result {
        CheckPackageInterfaceResult::Ok(path) => {
            println!("ok interface: {}", path.display());
            Ok(())
        }
        CheckPackageInterfaceResult::UpToDate(path) => {
            println!("up-to-date interface: {}", path.display());
            Ok(())
        }
        CheckPackageInterfaceResult::Invalid {
            path,
            status,
            manifest_path,
            detail,
            stale_reasons,
        } => {
            let manifest_path = normalize_path(&manifest_path);
            let error_line = format!(
                "error: {command_label} interface artifact `{}` is {}",
                normalize_path(&path),
                status.label()
            );
            let package_note = format!("note: failing package manifest: {manifest_path}");
            let workspace_member_note = workspace_member_manifest_path.map(|path| {
                format!(
                    "note: failing workspace member manifest: {}",
                    normalize_path(path)
                )
            });
            let mut notes = vec![package_note.as_str()];
            if let Some(workspace_member_note) = workspace_member_note.as_deref() {
                notes.push(workspace_member_note);
            }
            let regenerate_command =
                format_emit_interface_regenerate_command(&manifest_path, changed_only);
            let hint_line = format!("hint: rerun `{regenerate_command}` to regenerate it");
            report_interface_artifact_failure(
                &error_line,
                detail.as_deref(),
                &stale_reasons,
                &notes,
                &hint_line,
            );
            Err(1)
        }
    }
}

fn report_interface_stale_reasons(stale_reasons: &[InterfaceArtifactStaleReason]) {
    for reason in stale_reasons {
        match reason {
            InterfaceArtifactStaleReason::ManifestNewer { path } => {
                eprintln!(
                    "reason: manifest newer than artifact: {}",
                    normalize_path(path)
                );
            }
            InterfaceArtifactStaleReason::SourceNewer { path } => {
                eprintln!(
                    "reason: source newer than artifact: {}",
                    normalize_path(path)
                );
            }
        }
    }
}

fn sync_reference_interfaces(
    path: &Path,
    visited: &mut BTreeSet<String>,
) -> Result<Vec<PathBuf>, u8> {
    let check_command_label = format_check_command_label(true);
    let manifest = load_project_manifest(path).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {} manifest `{}` does not declare `[package].name`",
                check_command_label,
                normalize_path(manifest_path)
            );
            report_package_check_manifest_failure(manifest_path, true);
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {check_command_label} {error}");
            report_package_check_manifest_failure(manifest_path, true);
        } else {
            eprintln!("error: {error}");
        }
        1
    })?;
    let mut result = ReferenceInterfaceSyncResult::default();
    sync_reference_interfaces_recursive(&manifest, visited, &mut result, &check_command_label);
    if result.failure_count > 0 {
        for path in &result.written {
            println!("wrote interface: {}", path.display());
        }
        eprintln!(
            "error: {check_command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }
    Ok(result.written)
}

fn sync_reference_interfaces_recursive(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
    result: &mut ReferenceInterfaceSyncResult,
    command_label: &str,
) {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return;
    }

    for reference in &manifest.references.packages {
        let (dependency_manifest, reference_manifest_path) =
            match load_reference_manifest_for_sync(manifest, reference, command_label) {
                Ok(result) => result,
                Err(_) => {
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        reference_manifest_path(manifest, reference),
                    );
                    continue;
                }
            };
        let interface_path = reference_interface_path_for_sync(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            command_label,
        );
        let interface_path = match interface_path {
            Ok(path) => path,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        if interface_artifact_status(&dependency_manifest, &interface_path)
            != InterfaceArtifactStatus::Valid
        {
            let emit_result = emit_package_interface_path(
                &dependency_manifest.manifest_path,
                None,
                command_label,
                false,
            );
            match emit_result {
                Ok(EmitPackageInterfaceResult::Wrote(path)) => result.written.push(path),
                Ok(EmitPackageInterfaceResult::UpToDate(_)) => {}
                Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceFailure { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_source_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::Code { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_no_sources_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &source_root,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::ManifestFailure { manifest_path, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_manifest_failure(
                        &manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceRootFailure { source_root, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_source_root_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &source_root,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_output_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &output_path,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
            }
        }
        sync_reference_interfaces_recursive(&dependency_manifest, visited, result, command_label);
    }
}

fn prepare_reference_interfaces_for_manifests_quiet(
    manifest_paths: &[PathBuf],
) -> Result<(), ReferenceInterfacePrepError> {
    if manifest_paths.is_empty() {
        return Ok(());
    }

    let mut visited = BTreeSet::new();
    let mut result = ReferenceInterfaceSyncQuietResult::default();
    for manifest_path in manifest_paths {
        let manifest = load_project_manifest(manifest_path).map_err(|error| {
            let failure =
                reference_interface_prep_project_failure(None, None, manifest_path, &error);
            ReferenceInterfacePrepError {
                failure_count: 1,
                first_failure_manifest: failure.manifest_path.clone(),
                first_failure: failure,
            }
        })?;
        sync_reference_interfaces_recursive_quiet(&manifest, &mut visited, &mut result);
    }

    if result.failure_count > 0 {
        return Err(ReferenceInterfacePrepError {
            failure_count: result.failure_count,
            first_failure_manifest: result.first_failure_manifest,
            first_failure: result
                .first_failure
                .expect("quiet dependency interface prep failure should record the first failure"),
        });
    }

    Ok(())
}

fn sync_reference_interfaces_recursive_quiet(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
    result: &mut ReferenceInterfaceSyncQuietResult,
) {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return;
    }

    for reference in &manifest.references.packages {
        let reference_manifest_path = reference_manifest_path(manifest, reference);
        let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
        let dependency_manifest = match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(manifest) => manifest,
            Err(error) => {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_project_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &error,
                    ),
                );
                continue;
            }
        };
        let interface_path = match default_interface_path(&dependency_manifest) {
            Ok(path) => path,
            Err(error) => {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_project_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &error,
                    ),
                );
                continue;
            }
        };
        if interface_artifact_status(&dependency_manifest, &interface_path)
            != InterfaceArtifactStatus::Valid
        {
            if let Err(error) =
                emit_package_interface_path_quiet(&dependency_manifest.manifest_path, None, false)
            {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_emit_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &dependency_manifest.manifest_path,
                        error,
                    ),
                );
            }
        }
        sync_reference_interfaces_recursive_quiet(&dependency_manifest, visited, result);
    }
}

fn reference_interface_prep_project_failure(
    owner_manifest_path: Option<&Path>,
    reference: Option<&str>,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) -> ReferenceInterfacePrepFailure {
    let (manifest_path, error_kind, message, source_root) =
        if let ql_project::ProjectError::ManifestNotFound { start } = error {
            (
                Some(reference_manifest_path.to_path_buf()),
                "manifest",
                format!(
                    "could not find `qlang.toml` starting from `{}`",
                    normalize_path(start)
                ),
                None,
            )
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(error)
        {
            (
                Some(manifest_path.to_path_buf()),
                "manifest",
                format!(
                    "manifest `{}` does not declare `[package].name`",
                    normalize_path(manifest_path)
                ),
                None,
            )
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
            (
                Some(reference_manifest_path.to_path_buf()),
                "package-source-root",
                format!(
                    "package source directory `{}` does not exist",
                    normalize_path(path)
                ),
                Some(path.clone()),
            )
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            (
                Some(manifest_path.to_path_buf()),
                "manifest",
                error.to_string(),
                None,
            )
        } else {
            (
                Some(reference_manifest_path.to_path_buf()),
                "manifest",
                error.to_string(),
                None,
            )
        };
    ReferenceInterfacePrepFailure {
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        reference: reference.map(str::to_owned),
        reference_manifest_path: reference_manifest_path.to_path_buf(),
        manifest_path,
        failure_kind: ReferenceInterfacePrepFailureKind::Project {
            error_kind,
            message,
            source_root,
        },
    }
}

fn reference_interface_prep_emit_failure(
    owner_manifest_path: Option<&Path>,
    reference: Option<&str>,
    reference_manifest_path: &Path,
    dependency_manifest_path: &Path,
    error: EmitPackageInterfaceError,
) -> ReferenceInterfacePrepFailure {
    let manifest_path = match &error {
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: output_manifest_path,
            ..
        } => output_manifest_path
            .clone()
            .or(Some(dependency_manifest_path.to_path_buf())),
        EmitPackageInterfaceError::ManifestFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        EmitPackageInterfaceError::NoSourceFilesFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        EmitPackageInterfaceError::SourceRootFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        _ => Some(dependency_manifest_path.to_path_buf()),
    };
    ReferenceInterfacePrepFailure {
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        reference: reference.map(str::to_owned),
        reference_manifest_path: reference_manifest_path.to_path_buf(),
        manifest_path,
        failure_kind: ReferenceInterfacePrepFailureKind::InterfaceEmit(error),
    }
}

fn record_reference_interface_prep_failure(
    result: &mut ReferenceInterfaceSyncQuietResult,
    failure: ReferenceInterfacePrepFailure,
) {
    result.failure_count += 1;
    let manifest_path = failure
        .manifest_path
        .clone()
        .unwrap_or_else(|| failure.reference_manifest_path.clone());
    record_reference_failure_manifest(&mut result.first_failure_manifest, manifest_path);
    if result.first_failure.is_none() {
        result.first_failure = Some(failure);
    }
}

fn prepare_reference_interfaces_for_manifests(
    manifest_paths: &[PathBuf],
    command_label: &str,
    report_writes: bool,
) -> Result<(), u8> {
    if manifest_paths.is_empty() {
        return Ok(());
    }

    let mut visited = BTreeSet::new();
    let mut result = ReferenceInterfaceSyncResult::default();
    for manifest_path in manifest_paths {
        let manifest = load_project_manifest(manifest_path).map_err(|error| {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    command_label,
                    normalize_path(manifest_path)
                );
            } else if let Some(manifest_path) =
                package_check_manifest_path_from_project_error(&error)
            {
                eprintln!("error: {command_label} {error}");
                eprintln!(
                    "note: failing package manifest: {}",
                    normalize_path(manifest_path)
                );
            } else {
                eprintln!("error: {command_label} {error}");
            }
            1
        })?;
        sync_reference_interfaces_recursive(&manifest, &mut visited, &mut result, command_label);
    }

    if result.failure_count > 0 {
        if report_writes {
            for path in &result.written {
                println!("wrote interface: {}", path.display());
            }
        }
        eprintln!(
            "error: {command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    if report_writes {
        for path in result.written {
            println!("wrote interface: {}", path.display());
        }
    }

    Ok(())
}

fn ensure_reference_interfaces_current(manifest: &ql_project::ProjectManifest) -> Result<(), u8> {
    let check_command_label = format_check_command_label(false);
    let result = ensure_reference_interfaces_current_recursive(manifest, &mut BTreeSet::new());
    if result.failure_count > 0 {
        eprintln!(
            "error: {check_command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }
    Ok(())
}

fn ensure_reference_interfaces_current_recursive(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
) -> ReferenceInterfaceCheckResult {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return ReferenceInterfaceCheckResult::default();
    }

    let mut result = ReferenceInterfaceCheckResult::default();
    for reference in &manifest.references.packages {
        let (dependency_manifest, reference_manifest_path) =
            match load_reference_manifest_for_interfaces(manifest, reference, false) {
                Ok(result) => result,
                Err(_) => {
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        reference_manifest_path(manifest, reference),
                    );
                    continue;
                }
            };
        let dependency_package = reference_package_name_for_interfaces(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            false,
        );
        let dependency_package = match dependency_package {
            Ok(name) => name,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        let interface_path = reference_interface_path_for_interfaces(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            false,
        );
        let interface_path = match interface_path {
            Ok(path) => path,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        let status = interface_artifact_status(&dependency_manifest, &interface_path);
        if status != InterfaceArtifactStatus::Valid {
            report_reference_interface_artifact_issue(
                &dependency_manifest,
                reference,
                &dependency_package,
                &manifest_path,
                &interface_path,
                status,
            );
            result.failure_count += 1;
            record_reference_failure_manifest(
                &mut result.first_failure_manifest,
                dependency_manifest.manifest_path.clone(),
            );
        }
        let nested_result =
            ensure_reference_interfaces_current_recursive(&dependency_manifest, visited);
        result.failure_count += nested_result.failure_count;
        if result.first_failure_manifest.is_none() {
            result.first_failure_manifest = nested_result.first_failure_manifest;
        }
    }

    result
}

fn record_reference_failure_manifest(slot: &mut Option<PathBuf>, path: PathBuf) {
    if slot.is_none() {
        *slot = Some(path);
    }
}

fn record_first_failing_path(slot: &mut Option<PathBuf>, path: &Path) {
    if slot.is_none() {
        *slot = Some(path.to_path_buf());
    }
}

fn load_reference_manifest_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    sync_interfaces: bool,
) -> Result<(ql_project::ProjectManifest, PathBuf), u8> {
    let reference_manifest_path = reference_manifest_path(owner_manifest, reference);
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_manifest =
        load_project_manifest(&manifest_dir.join(reference)).map_err(|error| {
            report_reference_manifest_issue(
                sync_interfaces,
                reference,
                &owner_manifest.manifest_path,
                &reference_manifest_path,
                &error,
            );
            1
        })?;
    Ok((dependency_manifest, reference_manifest_path))
}

fn load_reference_manifest_for_sync(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    command_label: &str,
) -> Result<(ql_project::ProjectManifest, PathBuf), u8> {
    let reference_manifest_path = reference_manifest_path(owner_manifest, reference);
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_manifest =
        load_project_manifest(&manifest_dir.join(reference)).map_err(|error| {
            report_reference_manifest_issue_for_command(
                command_label,
                reference,
                &owner_manifest.manifest_path,
                &reference_manifest_path,
                &error,
            );
            1
        })?;
    Ok((dependency_manifest, reference_manifest_path))
}

fn reference_manifest_path(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
) -> PathBuf {
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let reference_path = manifest_dir.join(reference);
    if reference_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return reference_path;
    }
    reference_path.join("qlang.toml")
}

fn reference_package_name_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
) -> Result<String, u8> {
    package_name(dependency_manifest)
        .map(str::to_owned)
        .map_err(|error| {
            report_reference_manifest_issue(
                sync_interfaces,
                reference,
                &owner_manifest.manifest_path,
                reference_manifest_path,
                &error,
            );
            1
        })
}

fn reference_interface_path_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
) -> Result<PathBuf, u8> {
    default_interface_path(dependency_manifest).map_err(|error| {
        report_reference_manifest_issue(
            sync_interfaces,
            reference,
            &owner_manifest.manifest_path,
            reference_manifest_path,
            &error,
        );
        1
    })
}

fn reference_interface_path_for_sync(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    command_label: &str,
) -> Result<PathBuf, u8> {
    default_interface_path(dependency_manifest).map_err(|error| {
        report_reference_manifest_issue_for_command(
            command_label,
            reference,
            &owner_manifest.manifest_path,
            reference_manifest_path,
            &error,
        );
        1
    })
}

fn report_reference_manifest_issue(
    sync_interfaces: bool,
    reference: &str,
    owner_manifest_path: &Path,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) {
    let check_command_label = format_check_command_label(sync_interfaces);
    eprintln!("error: {check_command_label} failed to load referenced package `{reference}`");
    eprintln!("detail: {error}");
    eprintln!(
        "note: failing reference manifest: {}",
        normalize_path(reference_manifest_path)
    );
    eprintln!(
        "hint: fix the reference in `{}` or repair `{}`",
        normalize_path(owner_manifest_path),
        normalize_path(reference_manifest_path)
    );
}

fn report_reference_manifest_issue_for_command(
    command_label: &str,
    reference: &str,
    owner_manifest_path: &Path,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) {
    eprintln!("error: {command_label} failed to load referenced package `{reference}`");
    eprintln!("detail: {error}");
    eprintln!(
        "note: failing reference manifest: {}",
        normalize_path(reference_manifest_path)
    );
    eprintln!(
        "hint: fix the reference in `{}` or repair `{}`",
        normalize_path(owner_manifest_path),
        normalize_path(reference_manifest_path)
    );
}

fn format_reference_interface_sync_note(owner_manifest_path: &Path, reference: &str) -> String {
    let owner_manifest_path = normalize_path(owner_manifest_path);
    format!("note: while syncing referenced package `{reference}` from `{owner_manifest_path}`")
}

fn test_target_manifest_paths(targets: &[TestTarget]) -> Vec<PathBuf> {
    let mut manifest_paths = Vec::new();
    for target in targets {
        let TestTargetKind::Smoke {
            package_manifest_path: Some(manifest_path),
            ..
        } = &target.kind
        else {
            continue;
        };
        if !manifest_paths.contains(manifest_path) {
            manifest_paths.push(manifest_path.clone());
        }
    }
    manifest_paths
}

fn workspace_member_manifest_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return path.to_path_buf();
    }

    path.join("qlang.toml")
}

fn report_reference_interface_artifact_issue(
    dependency_manifest: &ql_project::ProjectManifest,
    reference: &str,
    dependency_package: &str,
    owner_manifest_path: &Path,
    interface_path: &Path,
    status: InterfaceArtifactStatus,
) {
    let check_command_label = format_check_command_label(false);
    let error_line = match status {
        InterfaceArtifactStatus::Missing => format!(
            "error: {check_command_label} referenced package `{dependency_package}` is missing interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Unreadable => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has unreadable interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Invalid => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has invalid interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Stale => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has stale interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Valid => return,
    };
    let detail = match status {
        InterfaceArtifactStatus::Unreadable => {
            interface_artifact_status_detail(interface_path, InterfaceArtifactStatus::Unreadable)
        }
        InterfaceArtifactStatus::Invalid => {
            interface_artifact_status_detail(interface_path, InterfaceArtifactStatus::Invalid)
        }
        _ => None,
    };
    let stale_reasons = if status == InterfaceArtifactStatus::Stale {
        interface_artifact_stale_reasons(dependency_manifest, interface_path)
    } else {
        Vec::new()
    };
    let failing_manifest_note = format!(
        "note: failing referenced package manifest: {}",
        normalize_path(&dependency_manifest.manifest_path)
    );
    let owner_manifest_path = normalize_path(owner_manifest_path);
    let owner_note = format!(
        "note: while checking referenced package `{reference}` from `{owner_manifest_path}`"
    );
    let hint_line = format!(
        "hint: rerun `ql check --sync-interfaces {owner_manifest_path}` or regenerate `{dependency_package}` with `ql project emit-interface {}`",
        normalize_path(&dependency_manifest.manifest_path)
    );
    let notes = [failing_manifest_note.as_str(), owner_note.as_str()];

    report_interface_artifact_failure(
        &error_line,
        detail.as_deref(),
        &stale_reasons,
        &notes,
        &hint_line,
    );
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

fn should_use_project_build(path: &Path) -> bool {
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
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        normalized.to_string_lossy().replace('\\', "/")
    }
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn json_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for ch in value.chars() {
        match ch {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\u{08}' => rendered.push_str("\\b"),
            '\u{0C}' => rendered.push_str("\\f"),
            ch if ch.is_control() => {
                use std::fmt::Write as _;
                write!(rendered, "\\u{:04x}", ch as u32).expect("write escaped json control");
            }
            _ => rendered.push(ch),
        }
    }
    rendered.push('"');
    rendered
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
    let normalized_path = normalize_path(path);
    eprint!(
        "{}",
        render_diagnostics(Path::new(&normalized_path), source, diagnostics)
    );
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
    eprintln!("  ql check <file-or-dir> [--sync-interfaces] [--json]");
    eprintln!(
        "  ql build <file-or-dir> [--emit llvm-ir|asm|obj|exe|dylib|staticlib] [--profile debug|release|--release] [--package <name>] [--lib|--bin <name>|--target <path>] [-o <output>] [--emit-interface] [--header] [--header-surface exports|imports|both] [--header-output <output>] [--json]"
    );
    eprintln!(
        "  ql run <file-or-dir> [--profile debug|release|--release] [--package <name>] [--bin <name>|--target <path>] [-- <args...>]"
    );
    eprintln!(
        "  ql test <file-or-dir> [--profile debug|release|--release] [--package <name>] [--target <tests/...ql>] [--list] [--filter <substring>]"
    );
    eprintln!("  ql project targets [file-or-dir] [--json]");
    eprintln!("  ql project graph [file-or-dir] [--json]");
    eprintln!("  ql project lock [file-or-dir] [--check]");
    eprintln!("  ql project init [dir] [--workspace] [--name <package>]");
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
        ProjectTargetSelector, analyze_semantics, analyze_source, build_path, collect_ql_files,
        render_mir_path, render_ownership_path, render_runtime_requirements,
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
}
