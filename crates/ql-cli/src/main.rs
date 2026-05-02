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
use ql_ast::{
    CallArg, Expr, ExprKind, FunctionDecl, GlobalDecl, ItemKind, Module, Param, ReceiverKind,
    Visibility,
};
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_driver::{
    BuildArtifact, BuildCHeaderOptions, BuildEmit, BuildError, BuildOptions, BuildProfile,
    CHeaderError, CHeaderOptions, CHeaderSurface, ToolchainError, build_file,
    build_file_with_link_inputs, build_source_with_link_inputs, default_output_path, emit_c_header,
};
use ql_fmt::format_source;
use ql_parser::parse_source;
use ql_project::{
    BuildTarget, BuildTargetKind, InterfaceArtifactStaleReason, InterfaceArtifactStatus,
    ManifestBuildProfile, WorkspaceBuildTargets, collect_package_sources, default_interface_path,
    discover_package_build_targets, discover_workspace_build_targets,
    interface_artifact_stale_reasons, interface_artifact_status, interface_artifact_status_detail,
    load_interface_artifact, load_project_manifest, load_reference_manifests, package_name,
    package_source_root, render_manifest_with_added_local_dependency,
    render_manifest_with_removed_local_dependency, render_module_interface,
};
use ql_runtime::{collect_runtime_hook_signatures, collect_runtime_hooks};
use ql_span::locate;
use serde_json::{Value as JsonValue, json};

mod dependency_generic_bridge;
mod project_dependencies;
mod project_graph;
mod project_init;
mod project_lock;
mod project_members;
mod project_status;
mod project_targets;

use project_dependencies::{
    ProjectDependentMember, find_workspace_member_dependents, project_dependencies_path,
    project_dependents_path,
};
use project_graph::project_graph_path;
use project_lock::project_lock_path;
use project_members::{
    project_add_binary_target_path, project_add_existing_path, project_add_path,
    project_remove_path,
};
use project_status::project_status_path;
use project_targets::{
    ProjectTargetSelector, ProjectTargetSelectorKind, is_runnable_project_target,
    list_build_targets_path, list_runnable_targets_path, parse_project_target_selector_option,
    project_target_display_path, project_targets_path,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, select_workspace_build_targets,
};

const CLI_NAME: &str = "ql";
const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

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

    if is_version_command(&command) {
        if let Some(extra) = args.next() {
            eprintln!("error: `ql {command}` does not accept additional argument `{extra}`");
            return Err(1);
        }
        println!("{}", version_text(CLI_NAME));
        return Ok(());
    }

    match command.as_str() {
        "check" => {
            let remaining = args.collect::<Vec<_>>();
            let mut path = None;
            let mut sync_interfaces = false;
            let mut json = false;
            let mut package_name = None;
            let mut index = 0;
            while index < remaining.len() {
                match remaining[index].as_str() {
                    "--sync-interfaces" => {
                        sync_interfaces = true;
                    }
                    "--json" => {
                        json = true;
                    }
                    "--package" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql check --package` expects a package name");
                            return Err(1);
                        };
                        if package_name.is_some() {
                            eprintln!("error: `ql check` received multiple `--package` selectors");
                            return Err(1);
                        }
                        package_name = Some(value.to_owned());
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
                index += 1;
            }
            let Some(path) = path else {
                eprintln!("error: `ql check` expects a file or directory path");
                return Err(1);
            };
            check_path(
                Path::new(&path),
                sync_interfaces,
                json,
                package_name.as_deref(),
            )
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
            let mut list = false;
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
                    "--list" => {
                        list = true;
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
            if list {
                return list_build_targets_path(Path::new(&path), &selector, json);
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
            let mut json = false;
            let mut list = false;
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
                    "--json" => {
                        json = true;
                    }
                    "--list" => {
                        list = true;
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

            if list {
                return list_runnable_targets_path(Path::new(&path), &selector, json);
            }
            run_path(
                Path::new(&path),
                profile_override.unwrap_or_default(),
                profile_override.is_some(),
                &selector,
                &program_args,
                json,
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
                "status" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--package" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project status --package` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project status` received `--package` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--json" => {
                                json = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project status` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project status` argument `{other}`"
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
                    project_status_path(&path, package_name.as_deref(), json)
                }
                "targets" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut selector = ProjectTargetSelector::default();
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        if parse_project_target_selector_option(
                            "`ql project targets`",
                            &remaining,
                            &mut index,
                            &mut selector,
                        )? {
                            index += 1;
                            continue;
                        }

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
                    project_targets_path(&path, &selector, json)
                }
                "target" => {
                    let Some(target_subcommand) = args.next() else {
                        eprintln!("error: `ql project target` expects a subcommand");
                        return Err(1);
                    };

                    match target_subcommand.as_str() {
                        "add" => {
                            let remaining = args.collect::<Vec<_>>();
                            let mut path = None;
                            let mut target_package_name = None;
                            let mut binary_name = None;
                            let mut index = 0;

                            while index < remaining.len() {
                                match remaining[index].as_str() {
                                    "--package" => {
                                        index += 1;
                                        let Some(value) = remaining.get(index) else {
                                            eprintln!(
                                                "error: `ql project target add --package` expects a package name"
                                            );
                                            return Err(1);
                                        };
                                        if target_package_name.is_some() {
                                            eprintln!(
                                                "error: `ql project target add` received `--package` more than once"
                                            );
                                            return Err(1);
                                        }
                                        target_package_name = Some(value.clone());
                                    }
                                    "--bin" => {
                                        index += 1;
                                        let Some(value) = remaining.get(index) else {
                                            eprintln!(
                                                "error: `ql project target add --bin` expects a target name"
                                            );
                                            return Err(1);
                                        };
                                        if binary_name.is_some() {
                                            eprintln!(
                                                "error: `ql project target add` received `--bin` more than once"
                                            );
                                            return Err(1);
                                        }
                                        binary_name = Some(value.clone());
                                    }
                                    other if other.starts_with('-') => {
                                        eprintln!(
                                            "error: unknown `ql project target add` option `{other}`"
                                        );
                                        return Err(1);
                                    }
                                    other => {
                                        if path.is_some() {
                                            eprintln!(
                                                "error: unknown `ql project target add` argument `{other}`"
                                            );
                                            return Err(1);
                                        }
                                        path = Some(PathBuf::from(other));
                                    }
                                }

                                index += 1;
                            }

                            let Some(binary_name) = binary_name else {
                                eprintln!("error: `ql project target add` expects `--bin <name>`");
                                return Err(1);
                            };
                            let path = path
                                .or_else(|| env::current_dir().ok())
                                .unwrap_or_else(|| PathBuf::from("."));
                            project_add_binary_target_path(
                                &path,
                                target_package_name.as_deref(),
                                &binary_name,
                            )
                        }
                        other => {
                            eprintln!("error: unknown `ql project target` subcommand `{other}`");
                            Err(1)
                        }
                    }
                }
                "graph" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--package" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project graph --package` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project graph` received `--package` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
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
                    project_graph_path(&path, package_name.as_deref(), json)
                }
                "dependents" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project dependents --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project dependents` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--json" => {
                                json = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!(
                                    "error: unknown `ql project dependents` option `{other}`"
                                );
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project dependents` argument `{other}`"
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
                    project_dependents_path(&path, package_name.as_deref(), json)
                }
                "dependencies" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project dependencies --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project dependencies` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--json" => {
                                json = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!(
                                    "error: unknown `ql project dependencies` option `{other}`"
                                );
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project dependencies` argument `{other}`"
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
                    project_dependencies_path(&path, package_name.as_deref(), json)
                }
                "lock" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut check_only = false;
                    let mut json = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--check" => {
                                check_only = true;
                            }
                            "--json" => {
                                json = true;
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
                    project_lock_path(&path, check_only, json)
                }
                "emit-interface" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut output = None;
                    let mut package_name = None;
                    let mut changed_only = false;
                    let mut check_only = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--package" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project emit-interface --package` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project emit-interface` received `--package` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
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
                    project_emit_interface_path(
                        &path,
                        output.as_deref(),
                        package_name.as_deref(),
                        changed_only,
                        check_only,
                    )
                }
                "init" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut workspace = false;
                    let mut package_name = None;
                    let mut stdlib_path = None;
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
                            "--stdlib" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project init --stdlib` expects a stdlib workspace path"
                                    );
                                    return Err(1);
                                };
                                if stdlib_path.is_some() {
                                    eprintln!(
                                        "error: `ql project init` received `--stdlib` more than once"
                                    );
                                    return Err(1);
                                }
                                stdlib_path = Some(PathBuf::from(value));
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
                    project_init::project_init_path(
                        &path,
                        workspace,
                        package_name.as_deref(),
                        stdlib_path.as_deref(),
                    )
                }
                "add" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut existing_path = None;
                    let mut dependencies = Vec::new();
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project add` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--existing" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add --existing` expects a file or directory"
                                    );
                                    return Err(1);
                                };
                                if existing_path.is_some() {
                                    eprintln!(
                                        "error: `ql project add` received `--existing` more than once"
                                    );
                                    return Err(1);
                                }
                                existing_path = Some(PathBuf::from(value));
                            }
                            "--dependency" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add --dependency` expects a package name"
                                    );
                                    return Err(1);
                                };
                                dependencies.push(value.clone());
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project add` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!("error: unknown `ql project add` argument `{other}`");
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
                    if let Some(existing_path) = existing_path {
                        if package_name.is_some() {
                            eprintln!(
                                "error: `ql project add --existing` does not accept `--name`; package name comes from the existing manifest"
                            );
                            return Err(1);
                        }
                        if !dependencies.is_empty() {
                            eprintln!(
                                "error: `ql project add --existing` does not accept `--dependency`; existing manifests keep their current local dependencies"
                            );
                            return Err(1);
                        }
                        project_add_existing_path(&path, &existing_path)
                    } else {
                        let Some(package_name) = package_name else {
                            eprintln!("error: `ql project add` requires `--name <package>`");
                            return Err(1);
                        };
                        project_add_path(&path, &package_name, &dependencies)
                    }
                }
                "remove" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut package_name = None;
                    let mut cascade = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project remove --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project remove` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--cascade" => {
                                cascade = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!("error: unknown `ql project remove` option `{other}`");
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project remove` argument `{other}`"
                                    );
                                    return Err(1);
                                }
                                path = Some(PathBuf::from(other));
                            }
                        }

                        index += 1;
                    }

                    let Some(package_name) = package_name else {
                        eprintln!("error: `ql project remove` requires `--name <package>`");
                        return Err(1);
                    };

                    let path = path
                        .or_else(|| env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    project_remove_path(&path, &package_name, cascade)
                }
                "add-dependency" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut target_package_name = None;
                    let mut package_name = None;
                    let mut dependency_path = None;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--package" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add-dependency --package` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if target_package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project add-dependency` received `--package` more than once"
                                    );
                                    return Err(1);
                                }
                                target_package_name = Some(value.clone());
                            }
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add-dependency --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project add-dependency` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--path" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project add-dependency --path` expects a package path"
                                    );
                                    return Err(1);
                                };
                                if dependency_path.is_some() {
                                    eprintln!(
                                        "error: `ql project add-dependency` received `--path` more than once"
                                    );
                                    return Err(1);
                                }
                                dependency_path = Some(PathBuf::from(value));
                            }
                            other if other.starts_with('-') => {
                                eprintln!(
                                    "error: unknown `ql project add-dependency` option `{other}`"
                                );
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project add-dependency` argument `{other}`"
                                    );
                                    return Err(1);
                                }
                                path = Some(PathBuf::from(other));
                            }
                        }

                        index += 1;
                    }

                    if package_name.is_some() && dependency_path.is_some() {
                        eprintln!(
                            "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both"
                        );
                        return Err(1);
                    }
                    if package_name.is_none() && dependency_path.is_none() {
                        eprintln!(
                            "error: `ql project add-dependency` requires `--name <package>` or `--path <file-or-dir>`"
                        );
                        return Err(1);
                    };

                    let path = path
                        .or_else(|| env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    project_add_dependency_path(
                        &path,
                        target_package_name.as_deref(),
                        package_name.as_deref(),
                        dependency_path.as_deref(),
                    )
                }
                "remove-dependency" => {
                    let remaining = args.collect::<Vec<_>>();
                    let mut path = None;
                    let mut target_package_name = None;
                    let mut package_name = None;
                    let mut remove_all = false;
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "--package" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project remove-dependency --package` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if target_package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project remove-dependency` received `--package` more than once"
                                    );
                                    return Err(1);
                                }
                                target_package_name = Some(value.clone());
                            }
                            "--name" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql project remove-dependency --name` expects a package name"
                                    );
                                    return Err(1);
                                };
                                if package_name.is_some() {
                                    eprintln!(
                                        "error: `ql project remove-dependency` received `--name` more than once"
                                    );
                                    return Err(1);
                                }
                                package_name = Some(value.clone());
                            }
                            "--all" => {
                                remove_all = true;
                            }
                            other if other.starts_with('-') => {
                                eprintln!(
                                    "error: unknown `ql project remove-dependency` option `{other}`"
                                );
                                return Err(1);
                            }
                            other => {
                                if path.is_some() {
                                    eprintln!(
                                        "error: unknown `ql project remove-dependency` argument `{other}`"
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
                    let package_name = if remove_all {
                        if let Some(package_name) = package_name {
                            package_name
                        } else {
                            resolve_project_workspace_member_package_name(
                                &path,
                                None,
                                "`ql project remove-dependency --all`",
                            )?
                        }
                    } else {
                        let Some(package_name) = package_name else {
                            eprintln!(
                                "error: `ql project remove-dependency` requires `--name <package>`"
                            );
                            return Err(1);
                        };
                        package_name
                    };
                    project_remove_dependency_path(
                        &path,
                        target_package_name.as_deref(),
                        &package_name,
                        remove_all,
                    )
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

fn check_path(
    path: &Path,
    sync_interfaces: bool,
    json: bool,
    package_name: Option<&str>,
) -> Result<(), u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest_request_path = request_root.as_deref().unwrap_or(path);
    if let Some(package_name) = package_name {
        let Ok(manifest) = load_project_manifest(manifest_request_path) else {
            report_check_package_selector_requires_workspace_context(package_name);
            return Err(1);
        };
        if manifest.workspace.is_none() {
            report_check_package_selector_requires_workspace_context(package_name);
            return Err(1);
        }
    }
    let use_package_check = should_use_package_check(manifest_request_path)
        || (is_ql_source_file(path) && load_project_manifest(manifest_request_path).is_ok());
    if use_package_check {
        let check_command_label = format_check_command_label(sync_interfaces);
        let mut package_manifest_path = None;
        let mut json_report = None;
        if let Ok(manifest) = load_project_manifest(manifest_request_path) {
            if manifest.package.is_none() && manifest.workspace.is_some() {
                return check_workspace_manifest(
                    &manifest,
                    manifest_request_path,
                    sync_interfaces,
                    json,
                    package_name,
                );
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
    request_path: &Path,
    sync_interfaces: bool,
    json: bool,
    selected_package_name: Option<&str>,
) -> Result<(), u8> {
    let Some(_) = &manifest.workspace else {
        return Ok(());
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let check_command_label = format_check_command_label(sync_interfaces);
    let selected_members = select_workspace_members(
        manifest,
        request_path,
        selected_package_name,
        &check_command_label,
    )?;
    let mut sync_visited = BTreeSet::new();
    let mut synced_interfaces = BTreeSet::new();
    let mut failing_members = 0usize;
    let mut first_failing_member_manifest = None;
    let mut json_report = json
        .then(|| CheckJsonReport::new("workspace", sync_interfaces, Some(&manifest.manifest_path)));
    let mut json_supported_failure_only = true;

    for member in &selected_members {
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

fn select_workspace_members(
    manifest: &ql_project::ProjectManifest,
    request_path: &Path,
    package_name: Option<&str>,
    command_label: &str,
) -> Result<Vec<String>, u8> {
    let Some(workspace) = &manifest.workspace else {
        return Ok(Vec::new());
    };
    let Some(package_name) = package_name else {
        return Ok(workspace.members.clone());
    };
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: `{command_label}` {message}");
        return Err(1);
    }

    let matching_members = find_workspace_member_entries_by_package_name(manifest, package_name);
    if matching_members.is_empty() {
        let normalized_path = normalize_path(request_path);
        let rerun_command = format!("{} {normalized_path}", command_label.trim_matches('`'));
        eprintln!(
            "error: {command_label} package selector matched no workspace members under `{normalized_path}`"
        );
        eprintln!("note: selector: package `{package_name}`");
        eprintln!(
            "hint: rerun `{rerun_command}` to inspect all workspace members, or adjust `--package`"
        );
        return Err(1);
    }
    if matching_members.len() > 1 {
        let manifest_path = normalize_path(&manifest.manifest_path);
        let rendered_members = matching_members
            .iter()
            .map(|(member, member_manifest)| {
                format!("{member} ({})", normalize_path(member_manifest))
            })
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "error: {command_label} workspace manifest `{manifest_path}` contains multiple members for package `{package_name}`: {rendered_members}"
        );
        return Err(1);
    }

    Ok(vec![
        matching_members
            .into_iter()
            .next()
            .expect("non-empty workspace check package matches should contain one entry")
            .0,
    ])
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
            None,
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
            Some(&request.request_root_manifest_path),
        );
    }

    if selector.is_active() {
        if json {
            let mut report =
                BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
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
        let mut report =
            BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
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
        project_request_root: Option<&Path>,
        options: &BuildOptions,
        profile_overridden: bool,
        emit_interface: bool,
    ) -> Self {
        let direct_source_request = resolve_project_source_build_target_request(path);
        let project_scope = project_request_root.is_some()
            || should_use_project_build(path)
            || direct_source_request.is_some();
        let project_manifest_path = if let Some(request_root) = project_request_root {
            load_project_manifest(request_root)
                .ok()
                .map(|manifest| normalize_path(&manifest.manifest_path))
        } else if let Some(request) = direct_source_request.as_ref() {
            Some(normalize_path(&request.request_root_manifest_path))
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

#[derive(Debug)]
struct RunJsonReport {
    scope: &'static str,
    path: String,
    project_manifest_path: Option<String>,
    requested_profile: &'static str,
    profile_overridden: bool,
    program_args: Vec<String>,
    built_target: Option<JsonValue>,
    execution: Option<JsonValue>,
    failure: Option<JsonValue>,
}

impl RunJsonReport {
    fn new(
        path: &Path,
        project_request_root: Option<&Path>,
        options: &BuildOptions,
        profile_overridden: bool,
        program_args: &[String],
    ) -> Self {
        let build_report = BuildJsonReport::new(
            path,
            project_request_root,
            options,
            profile_overridden,
            false,
        );
        Self {
            scope: build_report.scope,
            path: build_report.path,
            project_manifest_path: build_report.project_manifest_path,
            requested_profile: build_report.requested_profile,
            profile_overridden: build_report.profile_overridden,
            program_args: program_args.to_vec(),
            built_target: None,
            execution: None,
            failure: None,
        }
    }

    fn record_source_target(&mut self, path: &Path, artifact: &BuildArtifact) {
        self.built_target = Some(build_json_target(
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
    ) {
        self.built_target = Some(build_json_target(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            artifact,
            true,
        ));
    }

    fn record_source_build_failure(&mut self, path: &Path, error: &BuildError) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_failure(
                None,
                None,
                "source",
                normalize_path(path),
                true,
                error,
            ),
        }));
    }

    fn record_project_build_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        error: &BuildError,
    ) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_failure(
                Some(&member.member_manifest_path),
                Some(member.package_name.as_str()),
                target.kind.as_str(),
                project_target_display_path(&member.member_manifest_path, &target.path),
                true,
                error,
            ),
        }));
    }

    fn record_project_target_prep_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        failure: &PrepareProjectTargetBuildError,
    ) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_target_prep_failure(member, target, true, failure),
        }));
    }

    fn record_spawn_failure(&mut self, executable_path: &Path, message: String) {
        self.failure = Some(json!({
            "kind": "spawn",
            "artifact_path": normalize_path(executable_path),
            "message": message,
        }));
    }

    fn record_run_failure(
        &mut self,
        executable_path: &Path,
        message: &str,
        stdout: &str,
        stderr: &str,
    ) {
        self.failure = Some(json!({
            "kind": "run",
            "artifact_path": normalize_path(executable_path),
            "message": message,
            "stdout": stdout,
            "stderr": stderr,
        }));
    }

    fn record_execution(&mut self, exit_code: i32, stdout: &str, stderr: &str) {
        self.execution = Some(json!({
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        }));
    }

    fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.run.v1",
            "path": self.path,
            "scope": self.scope,
            "project_manifest_path": self.project_manifest_path,
            "requested_profile": self.requested_profile,
            "profile_overridden": self.profile_overridden,
            "program_args": self.program_args,
            "status": if self.failure.is_some() { "failed" } else { "completed" },
            "built_target": self.built_target,
            "execution": self.execution,
            "failure": self.failure,
        }))
        .expect("run json report should serialize");
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
        PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-function-conflict",
            format!("found conflicting direct dependency public function imports for `{symbol}`"),
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
        PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path,
            dependency_package,
            source_path,
            message,
        } => (
            "dependency-source",
            message.clone(),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(normalize_path(source_path)),
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-function-local-conflict",
            format!(
                "cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-function-unsupported-generic",
            format!(
                "cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-type-conflict",
            format!("found conflicting direct dependency public type imports for `{symbol}`"),
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
        PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-type-local-conflict",
            format!(
                "cannot synthesize direct dependency public type bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-value-conflict",
            format!("found conflicting direct dependency public value imports for `{symbol}`"),
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
        PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-value-local-conflict",
            format!(
                "cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
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

fn load_workspace_build_targets_for_build_json_from_request_root(
    request_path: &Path,
    request_root: &Path,
) -> Result<Vec<WorkspaceBuildTargets>, JsonValue> {
    let manifest = load_project_manifest(request_root)
        .map_err(|error| build_json_project_error(request_path, &error, "manifest-load"))?;
    discover_workspace_build_targets(&manifest)
        .map_err(|error| build_json_project_error(request_path, &error, "target-discovery"))
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

fn run_path(
    path: &Path,
    profile: BuildProfile,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
    json: bool,
) -> Result<(), u8> {
    let options = run_build_options(profile);
    if json {
        if should_use_project_build(path) {
            return run_project_path_json(
                path,
                path,
                &options,
                profile_overridden,
                selector,
                program_args,
            );
        }

        if let Some(request) = resolve_project_source_build_target_request(path) {
            if selector.is_active() {
                report_project_source_path_rejects_target_selector("`ql run`", path, selector);
                return Err(1);
            }
            return run_project_path_json(
                path,
                &request.request_root_manifest_path,
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

        return run_path_json(path, &options, profile_overridden, program_args);
    }

    if should_use_project_build(path) {
        return run_project_path(
            path,
            &options,
            profile_overridden,
            selector,
            program_args,
            None,
        );
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
            Some(&request.request_root_manifest_path),
        );
    }

    if selector.is_active() {
        report_project_target_selector_requires_project_context("`ql run`", selector);
        return Err(1);
    }

    let artifact = build_single_source_target_silent(path, &options, false)?;
    run_built_executable(&artifact.path, program_args)
}

fn run_path_json(
    path: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    program_args: &[String],
) -> Result<(), u8> {
    let mut report = RunJsonReport::new(path, None, options, profile_overridden, program_args);
    match build_single_source_target_result(path, options) {
        Ok(artifact) => {
            report.record_source_target(path, &artifact);
            emit_run_json_execution(report, &artifact.path, program_args)
        }
        Err(error) => {
            report.record_source_build_failure(path, &error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
}

fn run_project_path_json(
    path: &Path,
    project_request_root: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
) -> Result<(), u8> {
    let mut report = RunJsonReport::new(
        path,
        Some(project_request_root),
        options,
        profile_overridden,
        program_args,
    );
    let all_members = load_workspace_build_targets_for_command_from_request_root(
        path,
        project_request_root,
        "`ql run`",
    )?;
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
    let build_plan =
        resolve_project_build_plan_members(&all_members, &runnable_members, "`ql run`")?;

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

    let report_member = WorkspaceBuildTargets {
        member_manifest_path: runnable.member_manifest_path.clone(),
        package_name: runnable.package_name.clone(),
        default_profile: runnable.default_profile,
        targets: vec![runnable.target.clone()],
    };

    match build_project_source_target_result(
        &build_plan,
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
    ) {
        Ok(artifact) => {
            report.record_project_target(&report_member, &runnable.target, &artifact);
            emit_run_json_execution(report, &artifact.path, program_args)
        }
        Err(BuildTargetJsonError::Early(error)) => {
            report.record_project_target_prep_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
        Err(BuildTargetJsonError::Build(error)) => {
            report.record_project_build_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
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
    project_request_root: Option<&Path>,
) -> Result<(), u8> {
    let request_root = project_request_root.unwrap_or(path);
    let all_members =
        load_workspace_build_targets_for_command_from_request_root(path, request_root, "`ql run`")?;
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
struct CapturedExecutableRun {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_built_executable_capture(
    executable_path: &Path,
    program_args: &[String],
) -> Result<CapturedExecutableRun, String> {
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let output = command.output().map_err(|error| {
        format!(
            "failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        )
    })?;

    Ok(CapturedExecutableRun {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn emit_run_json_execution(
    mut report: RunJsonReport,
    executable_path: &Path,
    program_args: &[String],
) -> Result<(), u8> {
    match run_built_executable_capture(executable_path, program_args) {
        Ok(captured) => {
            if let Some(exit_code) = captured.exit_code {
                report.record_execution(exit_code, &captured.stdout, &captured.stderr);
                print!("{}", report.into_json());
                if exit_code == 0 {
                    Ok(())
                } else {
                    std::process::exit(exit_code);
                }
            } else {
                report.record_run_failure(
                    executable_path,
                    &format!(
                        "built executable `{}` terminated without an exit code",
                        normalize_path(executable_path)
                    ),
                    &captured.stdout,
                    &captured.stderr,
                );
                print!("{}", report.into_json());
                Err(1)
            }
        }
        Err(error) => {
            report.record_spawn_failure(executable_path, error);
            print!("{}", report.into_json());
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
    let project_file_request = (!should_use_project_build(path))
        .then(|| resolve_project_file_test_request(path))
        .flatten();
    let discovered_targets = discover_test_targets(
        path,
        &build_options,
        command_options,
        project_file_request.as_ref(),
    )?;
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
        project_file_request
            .as_ref()
            .map(|request| request.request_root_manifest_path.as_path()),
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
    project_file_request: Option<&ProjectFileTestRequest>,
) -> Result<Vec<TestTarget>, u8> {
    if should_use_project_build(path) {
        discover_project_test_targets(
            path,
            path,
            options,
            command_options.package_name.as_deref(),
            command_options.profile_overridden,
        )
    } else if let Some(request) = project_file_request {
        let discovered = discover_project_test_targets(
            path,
            &request.request_root_manifest_path,
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
    request_root_manifest_path: PathBuf,
    display_path: String,
}

#[derive(Clone, Debug)]
struct ProjectSourceBuildTargetRequest {
    request_root_manifest_path: PathBuf,
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
    request_path: &Path,
    project_path: &Path,
    options: &BuildOptions,
    package_name: Option<&str>,
    profile_overridden: bool,
) -> Result<Vec<TestTarget>, u8> {
    let members = load_workspace_build_targets_for_command_from_request_root(
        request_path,
        project_path,
        "`ql test`",
    )?;
    let members = select_workspace_test_members(request_path, members, package_name)?;
    let request_root = project_request_root(project_path);
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
    let request_root_manifest_path = resolve_project_member_request_root(&manifest_path);
    let request_root = project_request_root(&request_root_manifest_path);

    Some(ProjectFileTestRequest {
        request_root_manifest_path,
        display_path: display_relative_to_root(&request_root, path),
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

    let request_root_manifest_path = resolve_project_member_request_root(&manifest.manifest_path);

    Some(ProjectSourceBuildTargetRequest {
        request_root_manifest_path,
        selector: ProjectTargetSelector {
            package_name: Some(package_name),
            target: Some(ProjectTargetSelectorKind::DisplayPath(display_path)),
        },
    })
}

fn resolve_project_member_request_root(package_manifest_path: &Path) -> PathBuf {
    find_enclosing_workspace_manifest_for_member(package_manifest_path)
        .unwrap_or_else(|| package_manifest_path.to_path_buf())
}

fn find_enclosing_workspace_manifest_for_member(package_manifest_path: &Path) -> Option<PathBuf> {
    let package_root = package_manifest_path.parent()?;
    let mut current = package_root.parent().map(Path::to_path_buf);

    while let Some(directory) = current {
        let candidate = directory.join("qlang.toml");
        if candidate.is_file()
            && let Ok(manifest) = load_project_manifest(&candidate)
            && workspace_manifest_contains_member(&manifest, package_manifest_path)
        {
            return Some(manifest.manifest_path);
        }
        current = directory.parent().map(Path::to_path_buf);
    }

    None
}

fn workspace_manifest_contains_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest_path: &Path,
) -> bool {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return false;
    };

    let expected_manifest_path = normalize_path(package_manifest_path);
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    workspace.members.iter().any(|member| {
        load_project_manifest(&workspace_root.join(member))
            .ok()
            .is_some_and(|member_manifest| {
                normalize_path(&member_manifest.manifest_path) == expected_manifest_path
            })
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
    project_request_root: Option<&Path>,
    targets: &[TestTarget],
    json: bool,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<TestExecutionReport, u8> {
    let manifest_paths = test_target_manifest_paths(targets);
    let workspace_members = if !manifest_paths.is_empty() {
        prepare_reference_interfaces_for_manifests(&manifest_paths, "`ql test`", false)?;
        let request_root = project_request_root.unwrap_or(path);
        let workspace_members = load_workspace_build_targets_for_command_from_request_root(
            path,
            request_root,
            "`ql test`",
        )?;
        let selected_members =
            select_project_build_plan_root_members(&workspace_members, &manifest_paths);
        prepare_project_dependency_builds(
            &workspace_members,
            &selected_members,
            "`ql test`",
            options,
            profile_overridden,
        )?;
        prepare_project_test_package_builds(
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
                    build_project_test_source_target_quiet(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
                    )
                } else {
                    build_project_test_source_target_silent(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
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

fn report_check_package_selector_requires_workspace_context(package_name: &str) {
    eprintln!("error: `ql check` package selectors require a workspace path");
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
    project_request_root: Option<&Path>,
) -> Result<(), u8> {
    let mut json_report = json.then(|| {
        BuildJsonReport::new(
            path,
            project_request_root,
            options,
            profile_overridden,
            emit_interface,
        )
    });
    let request_root = project_request_root.unwrap_or(path);

    let members = if json {
        match load_workspace_build_targets_for_build_json_from_request_root(path, request_root) {
            Ok(members) => members,
            Err(failure) => return emit_build_json_failure(&mut json_report, failure),
        }
    } else {
        load_workspace_build_targets_for_command_from_request_root(
            path,
            request_root,
            "`ql build`",
        )?
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
                !plan_member.require_targets && first_target.kind == BuildTargetKind::Library,
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
                !plan_member.require_targets && first_target.kind == BuildTargetKind::Library,
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
                    !plan_member.require_targets && target.kind == BuildTargetKind::Library,
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
                    !plan_member.require_targets && target.kind == BuildTargetKind::Library,
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
    DependencySource {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        source_path: PathBuf,
        message: String,
    },
    DependencyExternConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyFunctionConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyFunctionLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyFunctionUnsupportedGeneric {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyTypeConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyTypeLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyValueConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyValueLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
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
    let dependency_manifest_paths = dependency_manifest_paths_for_selected_roots(
        workspace_members,
        selected_members,
        command_label,
    )?;
    for plan_member in &build_plan {
        let manifest_path = normalize_path(&plan_member.member.member_manifest_path);
        if !dependency_manifest_paths.contains(&manifest_path)
            || plan_member.member.targets.is_empty()
        {
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
                target.kind == BuildTargetKind::Library,
            )?;
        }
    }
    Ok(())
}

fn prepare_project_test_package_builds(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<(), u8> {
    let build_plan =
        resolve_project_build_plan_members(workspace_members, selected_members, command_label)?;
    for plan_member in &build_plan {
        if !plan_member.require_targets {
            continue;
        }
        for target in &plan_member.member.targets {
            if target.kind != BuildTargetKind::Library {
                continue;
            }
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
                true,
            )?;
        }
    }
    Ok(())
}

fn dependency_manifest_paths_for_selected_roots(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
) -> Result<BTreeSet<String>, u8> {
    let mut dependency_manifest_paths = BTreeSet::new();
    for selected_member in selected_members {
        let selected_plan = resolve_project_build_plan_members(
            workspace_members,
            std::slice::from_ref(selected_member),
            command_label,
        )?;
        for plan_member in selected_plan {
            if !plan_member.require_targets {
                dependency_manifest_paths
                    .insert(normalize_path(&plan_member.member.member_manifest_path));
            }
        }
    }
    Ok(dependency_manifest_paths)
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

#[derive(Clone, Debug, Default)]
struct RenderedDependencyBridgeItems {
    declarations: String,
    source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
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
    include_public_function_exports: bool,
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
        include_public_function_exports,
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
    include_public_function_exports: bool,
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
        include_public_function_exports,
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
    include_public_function_exports: bool,
) -> Result<BuildArtifact, BuildTargetJsonError> {
    let prepared = prepare_project_target_build_quiet(
        build_plan,
        manifest_path,
        path,
        dependency_options,
        profile_overridden,
        include_public_function_exports,
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

fn build_project_test_source_target_silent(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<BuildArtifact, u8> {
    build_project_test_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        true,
    )
}

fn build_project_test_source_target_quiet(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<BuildArtifact, u8> {
    build_project_test_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        false,
    )
}

fn build_project_test_source_target_impl(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    let prepared = prepare_project_test_target_build(
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
        false,
        false,
        report_failure,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
}

fn prepare_project_target_build_quiet(
    build_plan: &[ProjectBuildPlanMember],
    manifest_path: &Path,
    path: &Path,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    include_public_function_exports: bool,
) -> Result<PreparedProjectTargetBuild, PrepareProjectTargetBuildError> {
    let additional_link_inputs =
        project_dependency_link_inputs(build_plan, dependency_options, profile_overridden);
    let source = fs::read_to_string(path).map_err(|error| PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::SourceRead {
            path: path.to_path_buf(),
            message: format!("failed to access `{}`: {error}", normalize_path(path)),
        },
    })?;
    let dependency_bridge_items =
        render_direct_dependency_bridge_items_quiet(manifest_path, &source)?;
    let public_function_exports = if include_public_function_exports {
        render_public_dependency_function_export_wrappers_quiet(manifest_path, &source)?
    } else {
        String::new()
    };
    let bridge_code = join_dependency_bridge_sections(
        &dependency_bridge_items.declarations,
        &public_function_exports,
    );

    let source_override = dependency_bridge_source_override(
        &source,
        &bridge_code,
        &dependency_bridge_items.source_rewrites,
    );

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
    include_public_function_exports: bool,
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
        include_public_function_exports,
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
    include_public_function_exports: bool,
    report_failure: bool,
) -> Result<PreparedProjectTargetBuild, u8> {
    let selected_members =
        select_project_build_plan_root_members(workspace_members, &[manifest_path.to_path_buf()]);
    let build_plan =
        resolve_project_build_plan_members(workspace_members, &selected_members, command_label)?;
    let additional_link_inputs =
        project_dependency_link_inputs(&build_plan, dependency_options, profile_overridden);
    let source = fs::read_to_string(path).map_err(|error| {
        if report_failure {
            eprintln!(
                "error: failed to access `{}`: {error}",
                normalize_path(path)
            );
        }
        1
    })?;
    let dependency_bridge_items = match render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        &source,
        report_failure,
    ) {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    let public_function_exports = match if include_public_function_exports {
        render_public_dependency_function_export_wrappers(
            command_label,
            manifest_path,
            &source,
            report_failure,
        )
    } else {
        Ok(String::new())
    } {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    let bridge_code = join_dependency_bridge_sections(
        &dependency_bridge_items.declarations,
        &public_function_exports,
    );

    let source_override = dependency_bridge_source_override(
        &source,
        &bridge_code,
        &dependency_bridge_items.source_rewrites,
    );

    Ok(PreparedProjectTargetBuild {
        source_override,
        additional_link_inputs,
    })
}

fn prepare_project_test_target_build(
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
    let mut additional_link_inputs =
        project_dependency_link_inputs(&build_plan, dependency_options, profile_overridden);
    additional_link_inputs.extend(project_selected_library_link_inputs(
        &build_plan,
        dependency_options,
        profile_overridden,
    ));
    let source = fs::read_to_string(path).map_err(|error| {
        if report_failure {
            eprintln!(
                "error: failed to access `{}`: {error}",
                normalize_path(path)
            );
        }
        1
    })?;
    let dependency_bridge_items = match render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        &source,
        report_failure,
    ) {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    let package_bridge_items = match render_package_under_test_bridge_items(
        command_label,
        workspace_members,
        manifest_path,
        &source,
        report_failure,
    ) {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    let bridge_code = join_dependency_bridge_sections(
        &dependency_bridge_items.declarations,
        &package_bridge_items.declarations,
    );
    let mut source_rewrites = dependency_bridge_items.source_rewrites;
    source_rewrites.extend(package_bridge_items.source_rewrites);

    let source_override =
        dependency_bridge_source_override(&source, &bridge_code, &source_rewrites);

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

fn dependency_bridge_source_override(
    source: &str,
    dependency_declarations: &str,
    source_rewrites: &[dependency_generic_bridge::SourceRewrite],
) -> Option<String> {
    if dependency_declarations.is_empty() && source_rewrites.is_empty() {
        return None;
    }

    let rewritten_source = apply_dependency_source_rewrites(source, source_rewrites);
    if dependency_declarations.is_empty() {
        Some(rewritten_source)
    } else {
        Some(append_dependency_declarations(
            &rewritten_source,
            dependency_declarations,
        ))
    }
}

fn apply_dependency_source_rewrites(
    source: &str,
    source_rewrites: &[dependency_generic_bridge::SourceRewrite],
) -> String {
    let mut rewrites = source_rewrites.to_vec();
    rewrites.sort_by(|left, right| {
        right
            .span
            .start
            .cmp(&left.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
    });

    let mut rewritten = source.to_owned();
    let mut next_start = source.len();
    for rewrite in rewrites {
        if rewrite.span.start > rewrite.span.end
            || rewrite.span.end > source.len()
            || !source.is_char_boundary(rewrite.span.start)
            || !source.is_char_boundary(rewrite.span.end)
            || rewrite.span.end > next_start
        {
            continue;
        }
        rewritten.replace_range(rewrite.span.start..rewrite.span.end, &rewrite.replacement);
        next_start = rewrite.span.start;
    }
    rewritten
}

fn join_dependency_bridge_sections(primary: &str, secondary: &str) -> String {
    let mut sections = Vec::new();
    if !primary.trim().is_empty() {
        sections.push(primary.trim().to_owned());
    }
    if !secondary.trim().is_empty() {
        sections.push(secondary.trim().to_owned());
    }
    sections.join("\n\n")
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

fn project_selected_library_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    let mut outputs = Vec::new();
    for plan_member in build_plan.iter().rev() {
        if !plan_member.require_targets {
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

#[derive(Clone, Debug, Default)]
struct ImportedDependencyExterns {
    whole_paths: BTreeSet<Vec<String>>,
    symbols_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

fn collect_top_level_definition_names(module: &Module) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function) => {
                names.insert(function.name.clone());
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                names.insert(global.name.clone());
            }
            ItemKind::Struct(struct_decl) => {
                names.insert(struct_decl.name.clone());
            }
            ItemKind::Enum(enum_decl) => {
                names.insert(enum_decl.name.clone());
            }
            ItemKind::Trait(trait_decl) => {
                names.insert(trait_decl.name.clone());
            }
            ItemKind::TypeAlias(alias) => {
                names.insert(alias.name.clone());
            }
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    names.insert(function.name.clone());
                }
            }
            ItemKind::Impl(_) | ItemKind::Extend(_) => {}
        }
    }
    names
}

fn dependency_interface_module_import_paths(
    dependency_package: &str,
    modules: &[ql_project::InterfaceModule],
) -> BTreeSet<Vec<String>> {
    modules
        .iter()
        .map(|module| dependency_interface_module_import_path(dependency_package, &module.syntax))
        .collect()
}

fn dependency_interface_module_import_path(
    dependency_package: &str,
    module: &Module,
) -> Vec<String> {
    module
        .package
        .as_ref()
        .map(|package| package.path.segments.clone())
        .unwrap_or_else(|| dependency_package.split('.').map(str::to_owned).collect())
}

fn dependency_module_source_path(dependency_manifest_path: &Path, source_path: &str) -> PathBuf {
    dependency_manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(source_path)
}

fn collect_imported_dependency_externs(
    root_module: &Module,
    module_paths: &BTreeSet<Vec<String>>,
) -> ImportedDependencyExterns {
    let mut imported = ImportedDependencyExterns::default();

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if !module_paths.contains(&use_decl.prefix.segments) {
                continue;
            }
            let symbols = imported
                .symbols_by_module_path
                .entry(use_decl.prefix.segments.clone())
                .or_default();
            for item in group {
                symbols.insert(item.name.clone());
            }
            continue;
        }

        if module_paths.contains(&use_decl.prefix.segments)
            || module_paths
                .iter()
                .any(|module_path| module_path.starts_with(&use_decl.prefix.segments))
        {
            imported
                .whole_paths
                .insert(use_decl.prefix.segments.clone());
            continue;
        }

        if use_decl.prefix.segments.len() < 2 {
            continue;
        }

        let module_path = use_decl.prefix.segments[..use_decl.prefix.segments.len() - 1].to_vec();
        if !module_paths.contains(&module_path) {
            continue;
        }

        if let Some(symbol_name) = use_decl.prefix.segments.last() {
            imported
                .symbols_by_module_path
                .entry(module_path)
                .or_default()
                .insert(symbol_name.clone());
        }
    }

    imported
}

fn dependency_extern_is_imported(
    imported_externs: &ImportedDependencyExterns,
    module_path: &[String],
    symbol_name: &str,
) -> bool {
    imported_externs
        .whole_paths
        .iter()
        .any(|whole_path| module_path.starts_with(whole_path))
        || imported_externs
            .symbols_by_module_path
            .get(module_path)
            .is_some_and(|symbols| symbols.contains(symbol_name))
}

fn extend_dependency_bridge_name_requirements(
    destination: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    source: &BTreeMap<Vec<String>, BTreeSet<String>>,
) {
    for (module_path, symbols) in source {
        destination
            .entry(module_path.clone())
            .or_default()
            .extend(symbols.iter().cloned());
    }
}

fn render_direct_dependency_bridge_items(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let value_declarations = render_direct_dependency_public_value_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let function_forwarders = render_direct_dependency_public_function_forwarders(
        command_label,
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
        report_failure,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders(
        command_label,
        manifest_path,
        &required_types_by_module_path,
        report_failure,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations(
        command_label,
        manifest_path,
        source,
        &required_types_by_module_path,
        report_failure,
    )?;
    let extern_declarations = render_direct_dependency_extern_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

fn render_direct_dependency_bridge_items_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyBridgeItems, PrepareProjectTargetBuildError> {
    let value_declarations =
        render_direct_dependency_public_value_declarations_quiet(manifest_path, source)?;
    let function_forwarders = render_direct_dependency_public_function_forwarders_quiet(
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders_quiet(
        manifest_path,
        &required_types_by_module_path,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations_quiet(
        manifest_path,
        source,
        &required_types_by_module_path,
    )?;
    let extern_declarations =
        render_direct_dependency_extern_declarations_quiet(manifest_path, source)?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

struct PackageBridgeModule {
    source: String,
    module: Module,
}

fn package_under_test_bridge_modules(
    command_label: &str,
    member: &WorkspaceBuildTargets,
    report_failure: bool,
) -> Result<Vec<PackageBridgeModule>, u8> {
    let mut modules = Vec::new();
    for target in &member.targets {
        if target.kind != BuildTargetKind::Library {
            continue;
        }
        let source = fs::read_to_string(&target.path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to access package-under-test source `{}`: {error}",
                    normalize_path(&target.path)
                );
            }
            1
        })?;
        let module = parse_source(&source).map_err(|_| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to parse package-under-test source `{}` while preparing test bridges",
                    normalize_path(&target.path)
                );
            }
            1
        })?;
        modules.push(PackageBridgeModule { source, module });
    }
    Ok(modules)
}

fn render_package_under_test_bridge_items(
    command_label: &str,
    workspace_members: &[WorkspaceBuildTargets],
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let Some(member) = workspace_members.iter().find(|member| {
        normalize_path(&member.member_manifest_path) == normalize_path(manifest_path)
    }) else {
        return Ok(RenderedDependencyBridgeItems::default());
    };
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyBridgeItems::default()),
    };
    let bridge_modules = package_under_test_bridge_modules(command_label, member, report_failure)?;
    if bridge_modules.is_empty() {
        return Ok(RenderedDependencyBridgeItems::default());
    }

    let package_name = member.package_name.as_str();
    let module_import_paths = bridge_modules
        .iter()
        .map(|module| dependency_interface_module_import_path(package_name, &module.module))
        .collect::<BTreeSet<_>>();
    let imported_externs =
        collect_imported_dependency_externs(&root_source_module, &module_import_paths);
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut function_owners = BTreeMap::<String, DependencyExternOwner>::new();
    for module in &bridge_modules {
        collect_dependency_module_public_function_forwarders(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            &root_source_module,
            Some(&imported_externs),
            None,
            &occupied_root_names,
            &mut required_types_by_module_path,
            &mut function_owners,
            &mut forwarders,
            &mut source_rewrites,
        )
        .map_err(|error| {
            if !report_failure {
                return 1;
            }
            match error {
                DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
                    eprintln!(
                        "error: {command_label} found conflicting package-under-test public function imports for `{symbol}`"
                    );
                    eprintln!("note: first package: `{}`", owner.package_name);
                    eprintln!("note: package under test: `{package_name}`");
                }
                DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
                    eprintln!(
                        "error: {command_label} cannot synthesize package-under-test public function bridge for `{symbol}` because the test source already defines the same top-level name"
                    );
                    eprintln!(
                        "hint: rename the local top-level item or avoid importing the package-under-test public function with the same original symbol name"
                    );
                }
                DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
                    eprintln!(
                        "error: {command_label} cannot synthesize package-under-test public function bridge for generic function `{symbol}` yet"
                    );
                    eprintln!(
                        "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types"
                    );
                }
            }
            1
        })?;
    }

    let mut type_declarations = Vec::new();
    let mut type_owners = BTreeMap::<String, DependencyExternOwner>::new();
    for module in &bridge_modules {
        let module_import_path =
            dependency_interface_module_import_path(package_name, &module.module);
        collect_dependency_module_public_type_declarations(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            Some(&imported_externs),
            required_types_by_module_path.get(&module_import_path),
            &occupied_root_names,
            &mut type_owners,
            &mut type_declarations,
        )
        .map_err(|error| {
            if report_failure {
                match error {
                    DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                        eprintln!(
                            "error: {command_label} found conflicting package-under-test public type imports for `{symbol}`"
                        );
                        eprintln!("note: first package: `{}`", owner.package_name);
                        eprintln!("note: package under test: `{package_name}`");
                    }
                    DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                        eprintln!(
                            "error: {command_label} cannot synthesize package-under-test public type bridge for `{symbol}` because the test source already defines the same top-level name"
                        );
                        eprintln!(
                            "hint: rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name"
                        );
                    }
                }
            }
            1
        })?;
    }

    let declarations =
        join_dependency_bridge_sections(&type_declarations.join("\n\n"), &forwarders.join("\n\n"));
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites,
    })
}

fn render_direct_dependency_extern_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
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
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };

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
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
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
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency_manifest in direct_dependencies {
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
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
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

#[derive(Default)]
struct RenderedDependencyPublicValueDeclarations {
    declarations: String,
    required_functions_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

#[derive(Default)]
struct RenderedDependencyPublicFunctionForwarders {
    forwarders: String,
    source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

#[derive(Default)]
struct RenderedDependencyPublicMethodForwarders {
    forwarders: String,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

fn render_direct_dependency_public_value_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyPublicValueDeclarations, u8> {
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
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

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
                    "note: while preparing dependency public value bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let dependency_source = fs::read_to_string(&dependency_source_path).map_err(|error| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} failed to access dependency source `{}`: {error}",
                        normalize_path(&dependency_source_path)
                    );
                    eprintln!(
                        "note: while preparing dependency public value bridges for `{}`",
                        normalize_path(manifest_path)
                    );
                }
                1
            })?;
            let source_module = match parse_source(&dependency_source) {
                Ok(module) => module,
                Err(_) => {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public value bridges",
                            normalize_path(&dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    return Err(1);
                }
            };
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    match error {
                        DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public value imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public value names unique until package-qualified dependency value lowering lands"
                            );
                        }
                        DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!("note: conflicting direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public value with the same original symbol name"
                            );
                        }
                    }
                }
                1
            })?;
        }
    }

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_value_declarations_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyPublicValueDeclarations, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency_manifest in direct_dependencies {
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
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path = dependency_module_source_path(
                &dependency_manifest.manifest_path,
                &module.source_path,
            );
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.clone(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(&dependency_source_path)
                            ),
                        },
                    }
                })?;
            let source_module = parse_source(&dependency_source).map_err(|_| {
                PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.clone(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public value bridges",
                            normalize_path(&dependency_source_path)
                        ),
                    },
                }
            })?;
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| match error {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
                                symbol,
                                first_package: owner.package_name,
                                first_manifest_path: owner.manifest_path,
                                conflicting_package: dependency_package.clone(),
                                conflicting_manifest_path: dependency_manifest
                                    .manifest_path
                                    .clone(),
                            },
                    }
                }
                DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
                                symbol,
                                dependency_package: dependency_package.clone(),
                                dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            },
                    }
                }
            })?;
        }
    }

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_function_forwarders(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicFunctionForwarders, u8> {
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
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

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
                    "note: while preparing dependency public function wrappers for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public function wrappers for `{}`",
                            normalize_path(manifest_path)
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public function wrappers",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    1
                })
            },
            |error| {
                if report_failure {
                    match error {
                        DependencyPublicFunctionForwarderError::DependencyConflict {
                            symbol,
                            owner,
                        } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public function imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public function names unique until package-qualified dependency call lowering lands"
                            );
                        }
                        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!(
                                "note: conflicting direct dependency package: `{dependency_package}`"
                            );
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public function with the same original symbol name"
                            );
                        }
                        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
                            );
                            eprintln!("note: direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types"
                            );
                        }
                    }
                }
                1
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_function_forwarders_quiet(
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicFunctionForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency_manifest in direct_dependencies {
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
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.to_path_buf(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(dependency_source_path)
                            ),
                        },
                    }
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.to_path_buf(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public function wrappers",
                            normalize_path(dependency_source_path)
                        ),
                    },
                })
            },
            |error| {
                dependency_function_forwarder_target_prep_error(
                    error,
                    &dependency_package,
                    &dependency_manifest.manifest_path,
                )
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}

fn collect_dependency_public_function_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    root_source_module: &Module,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut map_bridge_error: impl FnMut(DependencyPublicFunctionForwarderError) -> E,
) -> Result<(), E> {
    let module_import_paths = dependency_interface_module_import_paths(dependency_package, modules);
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);

    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        let module_import_path =
            dependency_interface_module_import_path(dependency_package, &source_module);
        collect_dependency_module_public_function_forwarders(
            dependency_package,
            dependency_manifest_path,
            &source_module,
            &dependency_source,
            root_source_module,
            Some(&imported_externs),
            required_functions_by_module_path.get(&module_import_path),
            occupied_root_names,
            required_types_by_module_path,
            owners_by_symbol,
            forwarders,
            source_rewrites,
        )
        .map_err(&mut map_bridge_error)?;
    }

    Ok(())
}

fn render_direct_dependency_public_method_forwarders(
    command_label: &str,
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicMethodForwarders, u8> {
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

    let mut forwarders = Vec::new();
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

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
                    "note: while preparing dependency public method bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public method bridges for `{}`",
                            normalize_path(manifest_path)
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    1
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

fn render_direct_dependency_public_method_forwarders_quiet(
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicMethodForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut forwarders = Vec::new();
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency_manifest in direct_dependencies {
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

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.to_path_buf(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(dependency_source_path)
                            ),
                        },
                    }
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.to_path_buf(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        ),
                    },
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

fn collect_dependency_public_method_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
) -> Result<(), E> {
    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        collect_dependency_module_public_method_forwarders(
            dependency_package,
            &source_module,
            &dependency_source,
            required_types_by_module_path,
            discovered_required_types,
            forwarders,
        );
    }

    Ok(())
}

fn collect_dependency_module_public_method_forwarders(
    dependency_package: &str,
    module: &Module,
    contents: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
) {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let Some(initial_required_types) = required_types_by_module_path.get(&module_import_path)
    else {
        return;
    };
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut pending_types = initial_required_types.clone();
    let mut processed_types = BTreeSet::new();
    let mut emitted_methods = BTreeSet::<(String, String)>::new();

    while let Some(struct_name) = pending_types.iter().next().cloned() {
        pending_types.remove(&struct_name);
        if !processed_types.insert(struct_name.clone())
            || !type_candidates.contains_key(&struct_name)
        {
            continue;
        }
        if type_candidates
            .get(&struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }

        for (method_name, method) in
            dependency_public_struct_method_bridge_candidates(module, &struct_name)
        {
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(method, &type_candidates);
            if !type_dependencies.is_empty() {
                let required_types = discovered_required_types
                    .entry(module_import_path.clone())
                    .or_default();
                for dependency in type_dependencies {
                    if required_types.insert(dependency.clone()) {
                        pending_types.insert(dependency);
                    }
                }
            }

            if !emitted_methods.insert((struct_name.clone(), method_name)) {
                continue;
            }

            if let Some(forwarder) = render_imported_dependency_public_method_forwarder(
                &module_import_path,
                &struct_name,
                method,
                contents,
            ) {
                forwarders.push(forwarder);
            }
        }
    }
}

fn render_public_dependency_function_export_wrappers(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

fn render_public_dependency_function_export_wrappers_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let package_name = package_name(&manifest)
        .map_err(|error| target_prep_dependency_manifest_failure(Some(manifest_path), &error))?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

fn render_public_dependency_function_export_wrappers_for_package(
    package_name: &str,
    source: &str,
) -> String {
    let Ok(module) = parse_source(source) else {
        return String::new();
    };

    let mut wrappers = Vec::new();
    let module_import_path = dependency_interface_module_import_path(package_name, &module);
    let type_candidates = dependency_public_type_bridge_candidates(&module);
    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        if let Some(wrapper) =
            render_dependency_public_function_export_wrapper(&module_import_path, function, source)
        {
            wrappers.push(wrapper);
        }
    }
    for struct_name in type_candidates.keys() {
        if type_candidates
            .get(struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }
        for method in
            dependency_public_struct_method_bridge_candidates(&module, struct_name).values()
        {
            if let Some(wrapper) = render_dependency_public_method_export_wrapper(
                &module_import_path,
                struct_name,
                method,
                source,
            ) {
                wrappers.push(wrapper);
            }
        }
    }

    wrappers.join("\n\n")
}

fn collect_dependency_module_extern_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    _module_source_path: &str,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);

    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function)
                if function.visibility == Visibility::Public
                    && function.abi.as_deref() == Some("c")
                    && imported_externs.is_none_or(|imports| {
                        dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) =>
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
                    if imported_externs.is_some_and(|imports| {
                        !dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) {
                        continue;
                    }
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

#[derive(Clone, Copy)]
struct DependencyPublicGlobalBridgeCandidate<'a> {
    item: &'a ql_ast::Item,
    global: &'a GlobalDecl,
}

#[derive(Clone, Copy)]
struct DependencyPublicTypeBridgeCandidate<'a> {
    item: &'a ql_ast::Item,
    decl: DependencyPublicTypeDecl<'a>,
}

#[derive(Clone, Copy)]
enum DependencyPublicTypeDecl<'a> {
    Struct(&'a ql_ast::StructDecl),
    Enum(&'a ql_ast::EnumDecl),
    TypeAlias(&'a ql_ast::TypeAliasDecl),
}

impl<'a> DependencyPublicTypeDecl<'a> {
    fn name(self) -> &'a str {
        match self {
            Self::Struct(struct_decl) => struct_decl.name.as_str(),
            Self::Enum(enum_decl) => enum_decl.name.as_str(),
            Self::TypeAlias(alias) => alias.name.as_str(),
        }
    }

    fn is_generic(self) -> bool {
        match self {
            Self::Struct(struct_decl) => !struct_decl.generics.is_empty(),
            Self::Enum(enum_decl) => !enum_decl.generics.is_empty(),
            Self::TypeAlias(alias) => !alias.generics.is_empty(),
        }
    }
}

#[derive(Default)]
struct DependencyPublicGlobalExprDependencies {
    globals: BTreeSet<String>,
    functions: BTreeSet<String>,
}

fn dependency_public_global_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        candidates.insert(
            global.name.clone(),
            DependencyPublicGlobalBridgeCandidate { item, global },
        );
    }
    candidates
}

fn dependency_public_type_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let decl = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Struct(struct_decl)
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Enum(enum_decl)
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                DependencyPublicTypeDecl::TypeAlias(alias)
            }
            _ => continue,
        };
        candidates.insert(
            decl.name().to_owned(),
            DependencyPublicTypeBridgeCandidate { item, decl },
        );
    }
    candidates
}

fn dependency_public_function_bridge_candidates(module: &Module) -> BTreeSet<String> {
    module
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Function(function)
                if supports_dependency_public_function_export_bridge(function) =>
            {
                Some(function.name.clone())
            }
            _ => None,
        })
        .collect()
}

fn dependency_type_expr_targets_struct(ty: &ql_ast::TypeExpr, struct_name: &str) -> bool {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment == struct_name)
}

fn dependency_public_struct_method_bridge_candidates<'a>(
    module: &'a Module,
    struct_name: &str,
) -> BTreeMap<String, &'a FunctionDecl> {
    let mut impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut extend_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut trait_impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();

    for item in &module.items {
        match &item.kind {
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_none()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_some()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    trait_impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Extend(extend_block)
                if dependency_type_expr_targets_struct(&extend_block.target, struct_name) =>
            {
                for method in extend_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    extend_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            _ => {}
        }
    }

    let mut methods = BTreeMap::new();
    for (name, candidates) in impl_candidates {
        if candidates.len() == 1 {
            methods.insert(name, candidates.into_iter().next().unwrap());
        }
    }
    for (name, candidates) in extend_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    for (name, candidates) in trait_impl_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    methods
}

fn collect_dependency_public_type_expr_dependencies<'a>(
    ty: &ql_ast::TypeExpr,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    dependencies: &mut BTreeSet<String>,
) {
    match &ty.kind {
        ql_ast::TypeExprKind::Pointer { inner, .. } => {
            collect_dependency_public_type_expr_dependencies(inner, type_candidates, dependencies);
        }
        ql_ast::TypeExprKind::Array { element, .. } => {
            collect_dependency_public_type_expr_dependencies(
                element,
                type_candidates,
                dependencies,
            );
        }
        ql_ast::TypeExprKind::Named { path, args } => {
            if let [name] = path.segments.as_slice() {
                if type_candidates.contains_key(name) {
                    dependencies.insert(name.clone());
                }
            }
            for arg in args {
                collect_dependency_public_type_expr_dependencies(
                    arg,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            for item in items {
                collect_dependency_public_type_expr_dependencies(
                    item,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Callable { params, ret } => {
            for param in params {
                collect_dependency_public_type_expr_dependencies(
                    param,
                    type_candidates,
                    dependencies,
                );
            }
            collect_dependency_public_type_expr_dependencies(ret, type_candidates, dependencies);
        }
    }
}

fn dependency_public_type_dependencies<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<BTreeSet<String>> {
    let candidate = type_candidates.get(symbol_name)?;
    let mut dependencies = BTreeSet::new();
    match candidate.decl {
        DependencyPublicTypeDecl::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                collect_dependency_public_type_expr_dependencies(
                    &field.ty,
                    type_candidates,
                    &mut dependencies,
                );
            }
        }
        DependencyPublicTypeDecl::Enum(enum_decl) => {
            for variant in &enum_decl.variants {
                match &variant.fields {
                    ql_ast::VariantFields::Unit => {}
                    ql_ast::VariantFields::Tuple(items) => {
                        for item in items {
                            collect_dependency_public_type_expr_dependencies(
                                item,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                    ql_ast::VariantFields::Struct(fields) => {
                        for field in fields {
                            collect_dependency_public_type_expr_dependencies(
                                &field.ty,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                }
            }
        }
        DependencyPublicTypeDecl::TypeAlias(alias) => {
            collect_dependency_public_type_expr_dependencies(
                &alias.ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    Some(dependencies)
}

fn dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_type_bridge_order(
        symbol_name,
        type_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

fn collect_dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = type_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) = dependency_public_type_dependencies(symbol_name, type_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies {
        if !collect_dependency_public_type_bridge_order(
            &dependency,
            type_candidates,
            visiting,
            visited,
            ordered,
        ) {
            visiting.remove(symbol_name);
            return false;
        }
    }

    visiting.remove(symbol_name);
    visited.insert(symbol_name.to_owned());
    ordered.push(symbol_name.to_owned());
    true
}

fn collect_dependency_public_function_type_dependencies<'a>(
    function: &FunctionDecl,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    for param in &function.params {
        if let Param::Regular { ty, .. } = param {
            collect_dependency_public_type_expr_dependencies(
                ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    if let Some(return_type) = &function.return_type {
        collect_dependency_public_type_expr_dependencies(
            return_type,
            type_candidates,
            &mut dependencies,
        );
    }
    dependencies
}

fn collect_dependency_public_global_expr_dependencies<'a>(
    expr: &Expr,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    dependencies: &mut DependencyPublicGlobalExprDependencies,
) -> bool {
    match &expr.kind {
        ExprKind::Integer(_) | ExprKind::String { .. } | ExprKind::Bool(_) => true,
        ExprKind::Name(name) => {
            if global_candidates.contains_key(name) {
                dependencies.globals.insert(name.clone());
                true
            } else if function_candidates.contains(name) {
                dependencies.functions.insert(name.clone());
                true
            } else {
                false
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => items.iter().all(|item| {
            collect_dependency_public_global_expr_dependencies(
                item,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }),
        ExprKind::StructLiteral { fields, .. } => fields.iter().all(|field| {
            field.value.as_ref().is_some_and(|value| {
                collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }),
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_public_global_expr_dependencies(
                left,
                global_candidates,
                function_candidates,
                dependencies,
            ) && collect_dependency_public_global_expr_dependencies(
                right,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_public_global_expr_dependencies(
                expr,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Call { callee, args } => {
            collect_dependency_public_global_expr_dependencies(
                callee,
                global_candidates,
                function_candidates,
                dependencies,
            ) && args.iter().all(|arg| match arg {
                CallArg::Positional(value) => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
                CallArg::Named { value, .. } => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
            })
        }
        ExprKind::Member { object, .. } => collect_dependency_public_global_expr_dependencies(
            object,
            global_candidates,
            function_candidates,
            dependencies,
        ),
        ExprKind::Bracket { target, items } => {
            collect_dependency_public_global_expr_dependencies(
                target,
                global_candidates,
                function_candidates,
                dependencies,
            ) && items.iter().all(|item| {
                collect_dependency_public_global_expr_dependencies(
                    item,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }
        ExprKind::NoneLiteral
        | ExprKind::Block(_)
        | ExprKind::Unsafe(_)
        | ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::Closure { .. } => false,
    }
}

fn dependency_public_global_dependencies<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<DependencyPublicGlobalExprDependencies> {
    let candidate = global_candidates.get(symbol_name)?;
    let mut dependencies = DependencyPublicGlobalExprDependencies::default();
    collect_dependency_public_global_expr_dependencies(
        &candidate.global.value,
        global_candidates,
        function_candidates,
        &mut dependencies,
    )
    .then_some(dependencies)
}

fn dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_global_bridge_order(
        symbol_name,
        global_candidates,
        function_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

fn collect_dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = global_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) =
        dependency_public_global_dependencies(symbol_name, global_candidates, function_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies.globals {
        if !collect_dependency_public_global_bridge_order(
            &dependency,
            global_candidates,
            function_candidates,
            visiting,
            visited,
            ordered,
        ) {
            visiting.remove(symbol_name);
            return false;
        }
    }

    visiting.remove(symbol_name);
    visited.insert(symbol_name.to_owned());
    ordered.push(symbol_name.to_owned());
    true
}

fn collect_dependency_module_public_value_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    occupied_root_names: &BTreeSet<String>,
    required_functions_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicValueBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let global_candidates = dependency_public_global_bridge_candidates(module);
    let function_candidates = dependency_public_function_bridge_candidates(module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        if imported_externs.is_some_and(|imports| {
            !dependency_extern_is_imported(imports, &module_import_path, &global.name)
        }) {
            continue;
        }

        let Some(ordered_symbols) = dependency_public_global_bridge_order(
            &global.name,
            &global_candidates,
            &function_candidates,
        ) else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicValueBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = global_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public globals should resolve to candidates");
            let dependencies = dependency_public_global_dependencies(
                &ordered_symbol,
                &global_candidates,
                &function_candidates,
            )
            .expect("emitted dependency public globals should remain bridgeable");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner }
            })?;
            if !dependencies.functions.is_empty() {
                required_functions_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(dependencies.functions);
            }
            let mut type_dependencies = BTreeSet::new();
            collect_dependency_public_type_expr_dependencies(
                &candidate.global.ty,
                &type_candidates,
                &mut type_dependencies,
            );
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

fn collect_dependency_module_public_function_forwarders(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    root_module: &Module,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_function_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
) -> Result<(), DependencyPublicFunctionForwarderError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);

    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, &function.name)
        });
        let required_by_value = required_function_names.is_some_and(|required_function_names| {
            required_function_names.contains(&function.name)
        });
        if !imported && !required_by_value {
            continue;
        }
        if dependency_generic_bridge::supports_public_function_specialization(function) {
            let Some(rendered) = dependency_generic_bridge::render_public_function_specializations(
                &module_import_path,
                function,
                contents,
                root_module,
            ) else {
                return Err(DependencyPublicFunctionForwarderError::UnsupportedGeneric {
                    symbol: function.name.clone(),
                });
            };
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &function.name,
                rendered.declarations,
                owners_by_symbol,
                forwarders,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
            })?;
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(function, &type_candidates);
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            source_rewrites.extend(rendered.call_rewrites);
            continue;
        }
        if !supports_dependency_public_function_import_bridge(function) {
            continue;
        }
        let type_dependencies =
            collect_dependency_public_function_type_dependencies(function, &type_candidates);
        if occupied_root_names.contains(&function.name) {
            return Err(DependencyPublicFunctionForwarderError::LocalConflict {
                symbol: function.name.clone(),
            });
        }
        let Some(forwarder) = render_imported_dependency_public_function_forwarder(
            &module_import_path,
            function,
            contents,
        ) else {
            continue;
        };
        record_dependency_extern_declaration(
            dependency_package,
            dependency_manifest_path,
            &function.name,
            forwarder,
            owners_by_symbol,
            forwarders,
        )
        .map_err(|(symbol, owner)| {
            DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
        })?;
        if !type_dependencies.is_empty() {
            required_types_by_module_path
                .entry(module_import_path.clone())
                .or_default()
                .extend(type_dependencies);
        }
    }

    Ok(())
}

fn render_direct_dependency_public_type_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
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
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

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
                    "note: while preparing dependency public type bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let dependency_source = fs::read_to_string(&dependency_source_path).map_err(|error| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} failed to access dependency source `{}`: {error}",
                        normalize_path(&dependency_source_path)
                    );
                    eprintln!(
                        "note: while preparing dependency public type bridges for `{}`",
                        normalize_path(manifest_path)
                    );
                }
                1
            })?;
            let source_module = match parse_source(&dependency_source) {
                Ok(module) => module,
                Err(_) => {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public type bridges",
                            normalize_path(&dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    return Err(1);
                }
            };
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    match error {
                        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public type imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public type names unique until package-qualified dependency type lowering lands"
                            );
                        }
                        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public type bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!("note: conflicting direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public type with the same original symbol name"
                            );
                        }
                    }
                }
                1
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn render_direct_dependency_public_type_declarations_quiet(
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency_manifest in direct_dependencies {
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
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path = dependency_module_source_path(
                &dependency_manifest.manifest_path,
                &module.source_path,
            );
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.clone(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(&dependency_source_path)
                            ),
                        },
                    }
                })?;
            let source_module = parse_source(&dependency_source).map_err(|_| {
                PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.clone(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public type bridges",
                            normalize_path(&dependency_source_path)
                        ),
                    },
                }
            })?;
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| match error {
                DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
                                symbol,
                                first_package: owner.package_name,
                                first_manifest_path: owner.manifest_path,
                                conflicting_package: dependency_package.clone(),
                                conflicting_manifest_path: dependency_manifest
                                    .manifest_path
                                    .clone(),
                            },
                    }
                }
                DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
                                symbol,
                                dependency_package: dependency_package.clone(),
                                dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            },
                    }
                }
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn collect_dependency_module_public_type_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_type_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicTypeBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let type_name = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                struct_decl.name.as_str()
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                enum_decl.name.as_str()
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                alias.name.as_str()
            }
            _ => continue,
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, type_name)
        });
        let required_by_bridge = required_type_names
            .is_some_and(|required_type_names| required_type_names.contains(type_name));
        if !imported && !required_by_bridge {
            continue;
        }

        let Some(ordered_symbols) =
            dependency_public_type_bridge_order(type_name, &type_candidates)
        else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicTypeBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = type_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public types should resolve to candidates");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner }
            })?;
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

enum DependencyPublicTypeBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

enum DependencyPublicFunctionForwarderError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
    UnsupportedGeneric {
        symbol: String,
    },
}

fn dependency_function_forwarder_target_prep_error(
    error: DependencyPublicFunctionForwarderError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
    };
    PrepareProjectTargetBuildError { failure_kind }
}

enum DependencyPublicValueBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

fn supports_dependency_public_function_import_bridge(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public
        && function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && function.generics.is_empty()
        && function.where_clause.is_empty()
        && function
            .params
            .iter()
            .all(|param| matches!(param, Param::Regular { .. }))
}

fn supports_dependency_public_function_export_bridge(function: &FunctionDecl) -> bool {
    supports_dependency_public_function_import_bridge(function) && function.body.is_some()
}

fn render_imported_dependency_public_function_forwarder(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_function_import_bridge(function) {
        return None;
    }
    let params = render_dependency_bridge_param_list(function, contents);
    let args = render_dependency_bridge_arg_list(function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let callable_type = render_dependency_bridge_callable_type(function, contents);
    let export_name =
        dependency_public_function_export_name(module_import_path, function.name.as_str());
    let local_forwarder_name =
        dependency_public_function_local_forwarder_name(module_import_path, function.name.as_str());
    let mut rendered = format!("extern \"c\" fn {export_name}({params}){return_suffix}\n\n");
    rendered.push_str(&format!(
        "fn {local_forwarder_name}({params}){return_suffix} {{\n",
    ));
    if function.return_type.is_some() {
        rendered.push_str(&format!("    return {export_name}({args})\n"));
    } else {
        rendered.push_str(&format!("    {export_name}({args})\n"));
    }
    rendered.push_str("\n}\n\n");
    rendered.push_str(&format!(
        "const {}: {callable_type} = {local_forwarder_name}",
        function.name
    ));
    Some(rendered)
}

fn render_dependency_public_function_export_wrapper(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_function_export_bridge(function) {
        return None;
    }
    let params = render_dependency_bridge_param_list(function, contents);
    let args = render_dependency_bridge_arg_list(function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name =
        dependency_public_function_export_name(module_import_path, function.name.as_str());
    let mut rendered = format!("extern \"c\" pub fn {export_name}({params}){return_suffix} {{\n");
    if function.return_type.is_some() {
        rendered.push_str(&format!("    return {}({args})\n", function.name));
    } else {
        rendered.push_str(&format!("    {}({args})\n", function.name));
    }
    rendered.push('}');
    Some(rendered)
}

fn supports_dependency_public_method_import_bridge(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public
        && function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && function.generics.is_empty()
        && function.where_clause.is_empty()
        && matches!(function.params.first(), Some(Param::Receiver { .. }))
        && function
            .params
            .iter()
            .skip(1)
            .all(|param| matches!(param, Param::Regular { .. }))
}

fn supports_dependency_public_method_export_bridge(function: &FunctionDecl) -> bool {
    supports_dependency_public_method_import_bridge(function) && function.body.is_some()
}

fn render_imported_dependency_public_method_forwarder(
    module_import_path: &[String],
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_method_import_bridge(function) {
        return None;
    }
    let receiver_kind = dependency_bridge_receiver_kind(function)?;
    let ffi_params =
        render_dependency_bridge_method_ffi_param_list(struct_name, function, contents);
    let method_params = render_dependency_bridge_method_param_list(function, contents)?;
    let args = render_dependency_bridge_method_arg_list("self", function);
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name = dependency_public_method_export_name(
        module_import_path,
        struct_name,
        function.name.as_str(),
    );
    let visibility = render_dependency_bridge_visibility_prefix(&function.visibility);
    let mut rendered = format!("extern \"c\" fn {export_name}({ffi_params}){return_suffix}\n\n");
    rendered.push_str(&format!("impl {struct_name} {{\n"));
    rendered.push_str(&format!(
        "    {visibility}fn {}({method_params}){return_suffix} {{\n",
        function.name
    ));
    let _ = receiver_kind;
    if function.return_type.is_some() {
        rendered.push_str(&format!("        return {export_name}({args})\n"));
    } else {
        rendered.push_str(&format!("        {export_name}({args})\n"));
    }
    rendered.push_str("    }\n}");
    Some(rendered)
}

fn render_dependency_public_method_export_wrapper(
    module_import_path: &[String],
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    if !supports_dependency_public_method_export_bridge(function) {
        return None;
    }
    let receiver_kind = dependency_bridge_receiver_kind(function)?;
    let ffi_params =
        render_dependency_bridge_method_ffi_param_list(struct_name, function, contents);
    let regular_args = render_dependency_bridge_arg_list(function);
    let method_call_args = if regular_args.is_empty() {
        "()".to_owned()
    } else {
        format!("({regular_args})")
    };
    let receiver_name = match receiver_kind {
        ReceiverKind::Mutable => "bridge_receiver",
        ReceiverKind::ReadOnly | ReceiverKind::Move => "receiver",
    };
    let return_suffix = render_dependency_bridge_return_suffix(function, contents);
    let export_name = dependency_public_method_export_name(
        module_import_path,
        struct_name,
        function.name.as_str(),
    );
    let mut rendered =
        format!("extern \"c\" pub fn {export_name}({ffi_params}){return_suffix} {{\n");
    if matches!(receiver_kind, ReceiverKind::Mutable) {
        rendered.push_str("    var bridge_receiver = receiver\n");
    }
    if function.return_type.is_some() {
        rendered.push_str(&format!(
            "    return {receiver_name}.{}{method_call_args}\n",
            function.name
        ));
    } else {
        rendered.push_str(&format!(
            "    {receiver_name}.{}{method_call_args}\n",
            function.name
        ));
    }
    rendered.push('}');
    Some(rendered)
}

fn render_dependency_bridge_param_list(function: &FunctionDecl, contents: &str) -> String {
    function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, ty, .. } => {
                Some(format!("{name}: {}", span_text(contents, ty.span).trim()))
            }
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dependency_bridge_callable_type(function: &FunctionDecl, contents: &str) -> String {
    let params = function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { ty, .. } => Some(span_text(contents, ty.span).trim().to_owned()),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    let return_ty = function
        .return_type
        .as_ref()
        .map(|ty| span_text(contents, ty.span).trim().to_owned())
        .unwrap_or_else(|| "()".to_owned());
    format!("({params}) -> {return_ty}")
}

fn dependency_bridge_receiver_kind(function: &FunctionDecl) -> Option<ReceiverKind> {
    match function.params.first()? {
        Param::Receiver { kind, .. } => Some(*kind),
        Param::Regular { .. } => None,
    }
}

fn render_dependency_bridge_method_param_list(
    function: &FunctionDecl,
    contents: &str,
) -> Option<String> {
    let receiver =
        render_dependency_bridge_receiver_text(dependency_bridge_receiver_kind(function)?);
    let regular_params = render_dependency_bridge_param_list(function, contents);
    if regular_params.is_empty() {
        Some(receiver.to_owned())
    } else {
        Some(format!("{receiver}, {regular_params}"))
    }
}

fn render_dependency_bridge_method_ffi_param_list(
    struct_name: &str,
    function: &FunctionDecl,
    contents: &str,
) -> String {
    let regular_params = render_dependency_bridge_param_list(function, contents);
    if regular_params.is_empty() {
        format!("receiver: {struct_name}")
    } else {
        format!("receiver: {struct_name}, {regular_params}")
    }
}

fn render_dependency_bridge_method_arg_list(receiver: &str, function: &FunctionDecl) -> String {
    let regular_args = render_dependency_bridge_arg_list(function);
    if regular_args.is_empty() {
        receiver.to_owned()
    } else {
        format!("{receiver}, {regular_args}")
    }
}

fn render_dependency_bridge_arg_list(function: &FunctionDecl) -> String {
    function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, .. } => Some(name.clone()),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dependency_bridge_return_suffix(function: &FunctionDecl, contents: &str) -> String {
    function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", span_text(contents, ty.span).trim()))
        .unwrap_or_default()
}

fn render_dependency_bridge_receiver_text(kind: ReceiverKind) -> &'static str {
    match kind {
        ReceiverKind::ReadOnly => "self",
        ReceiverKind::Mutable => "var self",
        ReceiverKind::Move => "move self",
    }
}

fn render_dependency_bridge_visibility_prefix(visibility: &Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "",
        Visibility::Public => "pub ",
    }
}

fn dependency_public_function_export_name(
    module_import_path: &[String],
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

fn dependency_public_function_local_forwarder_name(
    module_import_path: &[String],
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_local_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

fn dependency_public_method_export_name(
    module_import_path: &[String],
    struct_name: &str,
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_method_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(struct_name));
    rendered.push('_');
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

fn sanitize_dependency_bridge_identifier_fragment(fragment: &str) -> String {
    let mut rendered = String::new();
    for ch in fragment.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            rendered.push(ch);
        } else {
            rendered.push('_');
        }
    }
    if rendered.is_empty() {
        rendered.push('_');
    }
    rendered
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

fn project_add_dependency_path(
    path: &Path,
    target_package_name: Option<&str>,
    package_name: Option<&str>,
    dependency_path: Option<&Path>,
) -> Result<(), u8> {
    let (workspace_manifest, package_manifest) = resolve_project_selected_package_manifest(
        path,
        target_package_name,
        "`ql project add-dependency`",
    )?;
    let dependency_entry = match (package_name, dependency_path) {
        (Some(package_name), None) => {
            if let Err(message) = validate_project_package_name(package_name) {
                eprintln!("error: `ql project add-dependency` {message}");
                return Err(1);
            }
            resolve_project_existing_dependency_entry(
                &workspace_manifest,
                &package_manifest,
                package_name,
            )
        }
        (None, Some(dependency_path)) => {
            resolve_project_path_dependency_entry(&package_manifest, dependency_path)
        }
        _ => Err("requires exactly one dependency selector".to_owned()),
    }
    .map_err(|message| {
        eprintln!("error: `ql project add-dependency` {message}");
        1
    })?;
    let package_manifest_source =
        fs::read_to_string(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project add-dependency` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
    let updated_package_manifest = render_manifest_with_added_local_dependency(
        &package_manifest_source,
        &dependency_entry.0,
        &dependency_entry.1,
    )
    .map_err(|message| {
        eprintln!("error: `ql project add-dependency` {message}");
        1
    })?;
    fs::write(&package_manifest.manifest_path, updated_package_manifest).map_err(|error| {
        eprintln!(
            "error: `ql project add-dependency` failed to write `{}`: {error}",
            normalize_path(&package_manifest.manifest_path)
        );
        1
    })?;

    println!(
        "updated: {}",
        normalize_path(&package_manifest.manifest_path)
    );
    Ok(())
}

fn project_remove_dependency_path(
    path: &Path,
    target_package_name: Option<&str>,
    package_name: &str,
    remove_all: bool,
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: `ql project remove-dependency` {message}");
        return Err(1);
    }

    if remove_all {
        if target_package_name.is_some() {
            eprintln!(
                "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`"
            );
            return Err(1);
        }
        return project_remove_dependency_from_all_workspace_members(path, package_name);
    }

    let (workspace_manifest, package_manifest) = resolve_project_selected_package_manifest(
        path,
        target_package_name,
        "`ql project remove-dependency`",
    )?;
    let dependency_entry = resolve_project_existing_dependency_entry(
        &workspace_manifest,
        &package_manifest,
        package_name,
    )
    .map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    let package_manifest_source =
        fs::read_to_string(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project remove-dependency` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
    let updated_package_manifest = render_manifest_with_removed_local_dependency(
        &package_manifest_source,
        &dependency_entry.0,
        &dependency_entry.1,
    )
    .map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    fs::write(&package_manifest.manifest_path, updated_package_manifest).map_err(|error| {
        eprintln!(
            "error: `ql project remove-dependency` failed to write `{}`: {error}",
            normalize_path(&package_manifest.manifest_path)
        );
        1
    })?;

    println!(
        "updated: {}",
        normalize_path(&package_manifest.manifest_path)
    );
    Ok(())
}

fn resolve_project_selected_package_manifest(
    path: &Path,
    target_package_name: Option<&str>,
    command_label: &str,
) -> Result<(ql_project::ProjectManifest, ql_project::ProjectManifest), u8> {
    let package_manifest = if let Some(target_package_name) = target_package_name {
        let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
            eprintln!("error: {command_label} {message}");
            1
        })?;
        if let Err(message) = validate_project_package_name(target_package_name) {
            eprintln!("error: {command_label} {message}");
            return Err(1);
        }

        let member_entries =
            find_workspace_member_entries_by_package_name(&workspace_manifest, target_package_name);
        if member_entries.is_empty() {
            eprintln!(
                "error: {command_label} workspace manifest `{}` does not contain package `{target_package_name}`",
                normalize_path(&workspace_manifest.manifest_path)
            );
            return Err(1);
        }
        if member_entries.len() > 1 {
            let matching_members = member_entries
                .iter()
                .map(|(member, _)| member.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!(
                "error: {command_label} workspace manifest `{}` contains multiple members for package `{target_package_name}`: {matching_members}",
                normalize_path(&workspace_manifest.manifest_path)
            );
            return Err(1);
        }

        load_project_manifest(&member_entries[0].1)
            .map_err(|error| {
                eprintln!("error: {command_label} {error}");
                1
            })
            .map(|package_manifest| (workspace_manifest, package_manifest))?
    } else {
        let package_manifest = resolve_project_package_manifest(path).map_err(|message| {
            eprintln!("error: {command_label} {message}");
            1
        })?;
        let workspace_manifest =
            resolve_project_workspace_manifest(path).unwrap_or_else(|_| package_manifest.clone());
        (workspace_manifest, package_manifest)
    };

    Ok(package_manifest)
}

fn project_remove_dependency_from_all_workspace_members(
    path: &Path,
    package_name: &str,
) -> Result<(), u8> {
    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    let dependency_entries =
        find_workspace_member_entries_by_package_name(&workspace_manifest, package_name);
    if dependency_entries.is_empty() {
        eprintln!(
            "error: `ql project remove-dependency` workspace manifest `{}` does not contain package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }
    if dependency_entries.len() > 1 {
        let matching_members = dependency_entries
            .iter()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "error: `ql project remove-dependency` workspace manifest `{}` contains multiple members for package `{package_name}`: {matching_members}",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }

    let member_manifest_path = &dependency_entries[0].1;
    let dependents = find_workspace_member_dependents(&workspace_manifest, member_manifest_path)
        .map_err(|message| {
            eprintln!("error: `ql project remove-dependency` {message}");
            1
        })?;
    if dependents.is_empty() {
        eprintln!(
            "error: `ql project remove-dependency` workspace package `{package_name}` does not have any dependent members to update in workspace manifest `{}`",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }

    let updated_dependency_manifests =
        detach_workspace_member_dependents(package_name, member_manifest_path, &dependents)
            .map_err(|message| {
                eprintln!("error: `ql project remove-dependency` {message}");
                1
            })?;
    for manifest_path in updated_dependency_manifests {
        println!("updated: {}", normalize_path(&manifest_path));
    }
    Ok(())
}

fn validate_project_package_name(package_name: &str) -> Result<(), String> {
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

fn absolute_user_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn resolve_project_workspace_manifest(path: &Path) -> Result<ql_project::ProjectManifest, String> {
    let manifest = load_project_manifest(path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;
    let workspace_manifest_path = if manifest.workspace.is_some() {
        manifest.manifest_path.clone()
    } else {
        resolve_project_member_request_root(&manifest.manifest_path)
    };
    let workspace_manifest = load_project_manifest(&workspace_manifest_path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;

    if workspace_manifest.workspace.is_none() {
        return Err(format!(
            "requires an existing workspace manifest; `{}` resolves to package manifest `{}`",
            normalize_path(path),
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }

    Ok(workspace_manifest)
}

fn resolve_project_package_manifest(path: &Path) -> Result<ql_project::ProjectManifest, String> {
    let manifest = load_project_manifest(path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;
    if manifest.package.is_none() {
        return Err(format!(
            "requires an existing package manifest; `{}` resolves to workspace manifest `{}`",
            normalize_path(path),
            normalize_path(&manifest.manifest_path)
        ));
    }
    Ok(manifest)
}

fn resolve_project_workspace_member_package_name(
    path: &Path,
    selected_package_name: Option<&str>,
    command_label: &str,
) -> Result<String, u8> {
    if let Some(package_name) = selected_package_name {
        if let Err(message) = validate_project_package_name(package_name) {
            eprintln!("error: {command_label} {message}");
            return Err(1);
        }
        return Ok(package_name.to_owned());
    }

    let package_manifest = load_project_manifest(path).map_err(|error| {
        eprintln!(
            "error: {command_label} {}",
            project_workspace_manifest_error(path, &error)
        );
        1
    })?;
    if package_manifest.package.is_none() {
        eprintln!(
            "error: {command_label} could not derive a package name from `{}`; rerun with `--name <package>`",
            normalize_path(path)
        );
        return Err(1);
    }

    package_name(&package_manifest)
        .map(str::to_owned)
        .map_err(|error| {
            eprintln!("error: {command_label} {error}");
            1
        })
}

fn project_workspace_manifest_error(path: &Path, error: &ql_project::ProjectError) -> String {
    match error {
        ql_project::ProjectError::ManifestNotFound { start } => format!(
            "requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        ),
        ql_project::ProjectError::PackageSourceRootNotFound {
            path: manifest_path,
        } => format!(
            "manifest `{}` does not have a project source root discoverable from `{}`",
            normalize_path(manifest_path),
            normalize_path(path)
        ),
        other => other.to_string(),
    }
}

fn find_workspace_member_with_package_name(
    workspace_manifest: &ql_project::ProjectManifest,
    wanted_package_name: &str,
) -> Option<PathBuf> {
    if workspace_manifest
        .package
        .as_ref()
        .is_some_and(|package| package.name == wanted_package_name)
    {
        return Some(workspace_manifest.manifest_path.clone());
    }

    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    workspace_manifest
        .workspace
        .as_ref()?
        .members
        .iter()
        .find_map(|member| {
            let member_manifest = load_project_manifest(&workspace_root.join(member)).ok()?;
            let existing_package_name = package_name(&member_manifest).ok()?;
            (existing_package_name == wanted_package_name).then_some(member_manifest.manifest_path)
        })
}

fn find_workspace_member_entries_by_package_name(
    workspace_manifest: &ql_project::ProjectManifest,
    wanted_package_name: &str,
) -> Vec<(String, PathBuf)> {
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    workspace_manifest
        .workspace
        .as_ref()
        .map(|workspace| {
            workspace
                .members
                .iter()
                .filter_map(|member| {
                    let member_manifest =
                        load_project_manifest(&workspace_root.join(member)).ok()?;
                    let existing_package_name = package_name(&member_manifest).ok()?;
                    (existing_package_name == wanted_package_name)
                        .then_some((member.clone(), member_manifest.manifest_path))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn detach_workspace_member_dependents(
    dependency_name: &str,
    dependency_manifest_path: &Path,
    dependents: &[ProjectDependentMember],
) -> Result<Vec<PathBuf>, String> {
    let dependency_root = dependency_manifest_path.parent().unwrap_or(Path::new("."));
    let mut updated_manifests = Vec::with_capacity(dependents.len());

    for dependent in dependents {
        let dependent_root = dependent.manifest_path.parent().unwrap_or(Path::new("."));
        let dependency_path = relative_path_from(dependent_root, dependency_root);
        let manifest_source = fs::read_to_string(&dependent.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}` while detaching dependent `{}`: {error}",
                normalize_path(&dependent.manifest_path),
                dependent.package_name
            )
        })?;
        let updated_manifest = render_manifest_with_removed_local_dependency(
            &manifest_source,
            dependency_name,
            &dependency_path,
        )
        .map_err(|message| {
            format!(
                "failed to detach local dependency from `{}`: {message}",
                normalize_path(&dependent.manifest_path)
            )
        })?;
        fs::write(&dependent.manifest_path, updated_manifest).map_err(|error| {
            format!(
                "failed to write `{}` while detaching dependent `{}`: {error}",
                normalize_path(&dependent.manifest_path),
                dependent.package_name
            )
        })?;
        updated_manifests.push(dependent.manifest_path.clone());
    }

    Ok(updated_manifests)
}

fn resolve_project_existing_dependency_entry(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest: &ql_project::ProjectManifest,
    dependency_name: &str,
) -> Result<(String, String), String> {
    let member_package_name = package_name(package_manifest)
        .map_err(|error| format!("failed to resolve current package name: {error}"))?;
    if dependency_name == member_package_name {
        return Err(format!(
            "does not accept self dependency `{dependency_name}` for package `{member_package_name}`"
        ));
    }

    let dependency_entries =
        find_workspace_member_entries_by_package_name(workspace_manifest, dependency_name);
    if dependency_entries.is_empty() {
        return Err(format!(
            "workspace manifest `{}` does not contain package `{dependency_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }
    if dependency_entries.len() > 1 {
        let matching_members = dependency_entries
            .iter()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "workspace manifest `{}` contains multiple members for package `{dependency_name}`: {matching_members}",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }

    let member_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_root = dependency_entries[0].1.parent().unwrap_or(Path::new("."));
    Ok((
        dependency_name.to_owned(),
        relative_path_from(member_root, dependency_root),
    ))
}

fn resolve_project_path_dependency_entry(
    package_manifest: &ql_project::ProjectManifest,
    dependency_path: &Path,
) -> Result<(String, String), String> {
    let member_package_name = package_name(package_manifest)
        .map_err(|error| format!("failed to resolve current package name: {error}"))?;
    let dependency_manifest =
        load_project_manifest(&absolute_user_path(dependency_path)).map_err(|error| {
            format!(
                "failed to resolve local dependency path `{}`: {error}",
                normalize_path(dependency_path)
            )
        })?;
    let dependency_name = package_name(&dependency_manifest)
        .map_err(|error| format!("failed to resolve dependency package name: {error}"))?;
    validate_project_package_name(dependency_name)?;

    if normalize_path(&dependency_manifest.manifest_path)
        == normalize_path(&package_manifest.manifest_path)
    {
        return Err(format!(
            "does not accept self dependency `{dependency_name}` for package `{member_package_name}`"
        ));
    }
    if dependency_name == member_package_name {
        return Err(format!(
            "does not accept dependency `{dependency_name}` because the current package has the same name"
        ));
    }

    let member_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_root = dependency_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    Ok((
        dependency_name.to_owned(),
        relative_path_from(member_root, dependency_root),
    ))
}

fn relative_path_from(from: &Path, to: &Path) -> String {
    let from = normalize_path(from);
    let to = normalize_path(to);
    let from_parts = from
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let to_parts = to
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();

    let mut common = 0;
    while common < from_parts.len()
        && common < to_parts.len()
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }

    if common == 0 && !from_parts.is_empty() && !to_parts.is_empty() && from_parts[0] != to_parts[0]
    {
        return to;
    }

    let mut relative = Vec::new();
    relative.extend(std::iter::repeat_n(
        "..",
        from_parts.len().saturating_sub(common),
    ));
    relative.extend_from_slice(&to_parts[common..]);
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative.join("/")
    }
}

fn stdlib_package_test_source() -> &'static str {
    r#"use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all3_bool as all3_bool
use std.core.all4_bool as all4_bool
use std.core.all5_bool as all5_bool
use std.core.and_bool as and_bool
use std.core.any3_bool as any3_bool
use std.core.any4_bool as any4_bool
use std.core.any5_bool as any5_bool
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.average4_int as average4_int
use std.core.average5_int as average5_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_max_int as clamp_max_int
use std.core.clamp_min_int as clamp_min_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.implies_bool as implies_bool
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.is_ascending_int as is_ascending_int
use std.core.is_descending_int as is_descending_int
use std.core.is_descending4_int as is_descending4_int
use std.core.is_descending5_int as is_descending5_int
use std.core.is_factor_of_int as is_factor_of_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_strictly_ascending_int as is_strictly_ascending_int
use std.core.is_ascending4_int as is_ascending4_int
use std.core.is_ascending5_int as is_ascending5_int
use std.core.is_strictly_ascending4_int as is_strictly_ascending4_int
use std.core.is_strictly_ascending5_int as is_strictly_ascending5_int
use std.core.is_strictly_descending4_int as is_strictly_descending4_int
use std.core.is_strictly_descending5_int as is_strictly_descending5_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.max4_int as max4_int
use std.core.max5_int as max5_int
use std.core.max_int as max_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.min4_int as min4_int
use std.core.min5_int as min5_int
use std.core.min_int as min_int
use std.core.none3_bool as none3_bool
use std.core.none4_bool as none4_bool
use std.core.none5_bool as none5_bool
use std.core.product3_int as product3_int
use std.core.product4_int as product4_int
use std.core.product5_int as product5_int
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.range_span_int as range_span_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.sum3_int as sum3_int
use std.core.sum4_int as sum4_int
use std.core.sum5_int as sum5_int
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.option.Option as Option
use std.option.none_option as option_none
use std.option.none_bool as none_bool
use std.option.none_int as none_int
use std.option.some_bool as some_bool
use std.option.some_int as some_int
use std.result.Result as Result
use std.result.err_bool as result_err_bool
use std.result.err_int as result_err_int
use std.result.ok_bool as result_ok_bool
use std.result.ok_int as result_ok_int
use std.test.expect_bool_all3 as expect_bool_all3
use std.test.expect_bool_all4 as expect_bool_all4
use std.test.expect_bool_all5 as expect_bool_all5
use std.test.expect_bool_and as expect_bool_and
use std.test.expect_bool_array_all3 as expect_bool_array_all3
use std.test.expect_bool_array_any5 as expect_bool_array_any5
use std.test.expect_bool_array_at5 as expect_bool_array_at5
use std.test.expect_bool_array_contains5 as expect_bool_array_contains5
use std.test.expect_bool_array_count5 as expect_bool_array_count5
use std.test.expect_bool_array_last5 as expect_bool_array_last5
use std.test.expect_bool_array_none4 as expect_bool_array_none4
use std.test.expect_bool_array_repeat5 as expect_bool_array_repeat5
use std.test.expect_bool_any3 as expect_bool_any3
use std.test.expect_bool_any4 as expect_bool_any4
use std.test.expect_bool_any5 as expect_bool_any5
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_bool_implies as expect_bool_implies
use std.test.expect_bool_ne as expect_bool_ne
use std.test.expect_bool_none3 as expect_bool_none3
use std.test.expect_bool_none4 as expect_bool_none4
use std.test.expect_bool_none5 as expect_bool_none5
use std.test.expect_bool_not as expect_bool_not
use std.test.expect_bool_option_none as expect_bool_option_none
use std.test.expect_bool_option_ok_or as expect_bool_option_ok_or
use std.test.expect_bool_option_ok_or_err as expect_bool_option_ok_or_err
use std.test.expect_bool_option_or as expect_bool_option_or
use std.test.expect_bool_option_some as expect_bool_option_some
use std.test.expect_bool_or as expect_bool_or
use std.test.expect_bool_result_err as expect_bool_result_err
use std.test.expect_bool_result_error_none as expect_bool_result_error_none
use std.test.expect_bool_result_error_some as expect_bool_result_error_some
use std.test.expect_bool_result_ok as expect_bool_result_ok
use std.test.expect_bool_result_or as expect_bool_result_or
use std.test.expect_bool_result_to_option_none as expect_bool_result_to_option_none
use std.test.expect_bool_result_to_option_some as expect_bool_result_to_option_some
use std.test.expect_bool_to_int as expect_bool_to_int
use std.test.expect_bool_xor as expect_bool_xor
use std.test.expect_false as expect_false
use std.test.expect_generic_bool_option_none as expect_generic_bool_option_none
use std.test.expect_generic_bool_option_ok_or as expect_generic_bool_option_ok_or
use std.test.expect_generic_bool_option_ok_or_err as expect_generic_bool_option_ok_or_err
use std.test.expect_generic_bool_option_or as expect_generic_bool_option_or
use std.test.expect_generic_bool_option_some as expect_generic_bool_option_some
use std.test.expect_generic_bool_result_err as expect_generic_bool_result_err
use std.test.expect_generic_bool_result_error as expect_generic_bool_result_error
use std.test.expect_generic_bool_result_error_none as expect_generic_bool_result_error_none
use std.test.expect_generic_bool_result_error_some as expect_generic_bool_result_error_some
use std.test.expect_generic_bool_result_ok as expect_generic_bool_result_ok
use std.test.expect_generic_bool_result_or as expect_generic_bool_result_or
use std.test.expect_generic_bool_result_to_option_none as expect_generic_bool_result_to_option_none
use std.test.expect_generic_bool_result_to_option_some as expect_generic_bool_result_to_option_some
use std.test.expect_generic_int_option_none as expect_generic_int_option_none
use std.test.expect_generic_int_option_ok_or as expect_generic_int_option_ok_or
use std.test.expect_generic_int_option_ok_or_err as expect_generic_int_option_ok_or_err
use std.test.expect_generic_int_option_or as expect_generic_int_option_or
use std.test.expect_generic_int_option_some as expect_generic_int_option_some
use std.test.expect_generic_int_result_err as expect_generic_int_result_err
use std.test.expect_generic_int_result_error as expect_generic_int_result_error
use std.test.expect_generic_int_result_error_none as expect_generic_int_result_error_none
use std.test.expect_generic_int_result_error_some as expect_generic_int_result_error_some
use std.test.expect_generic_int_result_ok as expect_generic_int_result_ok
use std.test.expect_generic_int_result_or as expect_generic_int_result_or
use std.test.expect_generic_int_result_to_option_none as expect_generic_int_result_to_option_none
use std.test.expect_generic_int_result_to_option_some as expect_generic_int_result_to_option_some
use std.test.expect_int_abs as expect_int_abs
use std.test.expect_int_abs_diff as expect_int_abs_diff
use std.test.expect_int_array_at3 as expect_int_array_at3
use std.test.expect_int_array_contains3 as expect_int_array_contains3
use std.test.expect_int_array_count3 as expect_int_array_count3
use std.test.expect_int_array_first3 as expect_int_array_first3
use std.test.expect_int_array_max5 as expect_int_array_max5
use std.test.expect_int_array_min5 as expect_int_array_min5
use std.test.expect_int_array_product4 as expect_int_array_product4
use std.test.expect_int_array_repeat3 as expect_int_array_repeat3
use std.test.expect_int_array_reverse3 as expect_int_array_reverse3
use std.test.expect_int_array_sum3 as expect_int_array_sum3
use std.test.expect_int_array_sum5 as expect_int_array_sum5
use std.test.expect_status_failed as expect_status_failed
use std.test.expect_status_ok as expect_status_ok
use std.test.expect_int_average2 as expect_int_average2
use std.test.expect_int_average3 as expect_int_average3
use std.test.expect_int_average4 as expect_int_average4
use std.test.expect_int_average5 as expect_int_average5
use std.test.expect_int_ascending as expect_int_ascending
use std.test.expect_int_ascending4 as expect_int_ascending4
use std.test.expect_int_ascending5 as expect_int_ascending5
use std.test.expect_int_between as expect_int_between
use std.test.expect_int_between_bounds as expect_int_between_bounds
use std.test.expect_int_clamp_max as expect_int_clamp_max
use std.test.expect_int_clamp_min as expect_int_clamp_min
use std.test.expect_int_clamped as expect_int_clamped
use std.test.expect_int_clamped_bounds as expect_int_clamped_bounds
use std.test.expect_int_compare as expect_int_compare
use std.test.expect_int_descending as expect_int_descending
use std.test.expect_int_descending4 as expect_int_descending4
use std.test.expect_int_descending5 as expect_int_descending5
use std.test.expect_int_distance_to_bounds as expect_int_distance_to_bounds
use std.test.expect_int_distance_to_range as expect_int_distance_to_range
use std.test.expect_int_divisible_by as expect_int_divisible_by
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_int_even as expect_int_even
use std.test.expect_int_exclusive_between_bounds as expect_int_exclusive_between_bounds
use std.test.expect_int_exclusive_between as expect_int_exclusive_between
use std.test.expect_int_factor_of as expect_int_factor_of
use std.test.expect_int_option_none as expect_int_option_none
use std.test.expect_int_option_ok_or as expect_int_option_ok_or
use std.test.expect_int_option_ok_or_err as expect_int_option_ok_or_err
use std.test.expect_int_option_or as expect_int_option_or
use std.test.expect_int_option_some as expect_int_option_some
use std.test.expect_int_result_err as expect_int_result_err
use std.test.expect_int_result_error_none as expect_int_result_error_none
use std.test.expect_int_result_error_some as expect_int_result_error_some
use std.test.expect_int_result_ok as expect_int_result_ok
use std.test.expect_int_result_or as expect_int_result_or
use std.test.expect_int_result_to_option_none as expect_int_result_to_option_none
use std.test.expect_int_result_to_option_some as expect_int_result_to_option_some
use std.test.expect_int_max as expect_int_max
use std.test.expect_int_max3 as expect_int_max3
use std.test.expect_int_max4 as expect_int_max4
use std.test.expect_int_max5 as expect_int_max5
use std.test.expect_int_median3 as expect_int_median3
use std.test.expect_int_min as expect_int_min
use std.test.expect_int_min3 as expect_int_min3
use std.test.expect_int_min4 as expect_int_min4
use std.test.expect_int_min5 as expect_int_min5
use std.test.expect_int_has_remainder as expect_int_has_remainder
use std.test.expect_int_negative as expect_int_negative
use std.test.expect_int_not_within as expect_int_not_within
use std.test.expect_int_nonnegative as expect_int_nonnegative
use std.test.expect_int_nonpositive as expect_int_nonpositive
use std.test.expect_int_lower_bound as expect_int_lower_bound
use std.test.expect_int_odd as expect_int_odd
use std.test.expect_int_outside as expect_int_outside
use std.test.expect_int_outside_bounds as expect_int_outside_bounds
use std.test.expect_int_positive as expect_int_positive
use std.test.expect_int_product3 as expect_int_product3
use std.test.expect_int_product4 as expect_int_product4
use std.test.expect_int_product5 as expect_int_product5
use std.test.expect_int_quotient_or_zero as expect_int_quotient_or_zero
use std.test.expect_int_range_span as expect_int_range_span
use std.test.expect_int_remainder_or_zero as expect_int_remainder_or_zero
use std.test.expect_int_sign as expect_int_sign
use std.test.expect_int_strictly_descending as expect_int_strictly_descending
use std.test.expect_int_strictly_ascending as expect_int_strictly_ascending
use std.test.expect_int_strictly_ascending4 as expect_int_strictly_ascending4
use std.test.expect_int_strictly_ascending5 as expect_int_strictly_ascending5
use std.test.expect_int_strictly_descending4 as expect_int_strictly_descending4
use std.test.expect_int_strictly_descending5 as expect_int_strictly_descending5
use std.test.expect_int_sum3 as expect_int_sum3
use std.test.expect_int_sum4 as expect_int_sum4
use std.test.expect_int_sum5 as expect_int_sum5
use std.test.expect_int_upper_bound as expect_int_upper_bound
use std.test.expect_int_within as expect_int_within
use std.test.expect_true as expect_true
use std.test.is_status_failed as is_status_failed
use std.test.is_status_ok as is_status_ok
use std.test.merge_status as merge_status
use std.test.merge_status3 as merge_status3
use std.test.merge_status4 as merge_status4
use std.test.merge_status5 as merge_status5
use std.test.merge_status6 as merge_status6

fn main() -> Int {
    let max_check = expect_int_eq(max_int(20, 22), 22)
    let max3_check = expect_int_eq(max3_int(20, 22, 21), 22)
    let max4_check = expect_int_eq(max4_int(20, 22, 21, 19), 22)
    let max5_check = expect_int_eq(max5_int(20, 22, 21, 19, 23), 23)
    let min_check = expect_int_eq(min_int(20, 22), 20)
    let min3_check = expect_int_eq(min3_int(20, 22, 21), 20)
    let min4_check = expect_int_eq(min4_int(20, 22, 21, 19), 19)
    let min5_check = expect_int_eq(min5_int(20, 22, 21, 19, 18), 18)
    let sum3_check = expect_int_eq(sum3_int(2, 3, 4), 9)
    let sum4_check = expect_int_eq(sum4_int(2, 3, 4, 5), 14)
    let sum5_check = expect_int_eq(sum5_int(2, 3, 4, 5, 6), 20)
    let product3_check = expect_int_eq(product3_int(2, 3, 4), 24)
    let product4_check = expect_int_eq(product4_int(2, 3, 4, 5), 120)
    let product5_check = expect_int_eq(product5_int(2, 3, 4, 5, 6), 720)
    let average2_check = expect_int_eq(average2_int(5, 8), 6)
    let average3_check = expect_int_eq(average3_int(3, 6, 9), 6)
    let average4_check = expect_int_eq(average4_int(2, 4, 6, 8), 5)
    let average5_check = expect_int_eq(average5_int(2, 4, 6, 8, 10), 6)
    let quotient_check = expect_int_eq(quotient_or_zero_int(21, 7), 3)
    let quotient_zero_check = expect_int_eq(quotient_or_zero_int(21, 0), 0)
    let remainder_check = expect_int_eq(remainder_or_zero_int(22, 7), 1)
    let remainder_zero_check = expect_int_eq(remainder_or_zero_int(22, 0), 0)
    let has_remainder_check = expect_bool_eq(has_remainder_int(22, 7), true)
    let factor_check = expect_bool_eq(is_factor_of_int(7, 21), true)
    let median3_check = expect_int_eq(median3_int(22, 20, 21), 21)
    let clamp_min_check = expect_int_eq(clamp_min_int(19, 20), 20)
    let clamp_max_check = expect_int_eq(clamp_max_int(23, 22), 22)
    let clamp_bounds_check = expect_int_eq(clamp_bounds_int(23, 22, 20), 22)
    let abs_check = expect_int_eq(abs_int(0 - 22), 22)
    let abs_diff_check = expect_int_eq(abs_diff_int(22, 19), 3)
    let range_span_check = expect_int_eq(range_span_int(22, 20), 2)
    let lower_bound_check = expect_int_eq(lower_bound_int(22, 20), 20)
    let upper_bound_check = expect_int_eq(upper_bound_int(22, 20), 22)
    let distance_range_check = expect_int_eq(distance_to_range_int(19, 20, 22), 1)
    let distance_bounds_check = expect_int_eq(distance_to_bounds_int(23, 22, 20), 1)
    let compare_check = expect_int_eq(compare_int(9, 3), 1)
    let sign_negative_check = expect_int_eq(sign_int(0 - 5), 0 - 1)
    let sign_zero_check = expect_int_eq(sign_int(0), 0)
    let sign_positive_check = expect_int_eq(sign_int(5), 1)
    let and_check = expect_false(and_bool(true, false))
    let xor_check = expect_bool_eq(xor_bool(true, false), true)
    let all3_check = expect_bool_eq(all3_bool(true, true, true), true)
    let all4_check = expect_bool_eq(all4_bool(true, true, true, false), false)
    let all5_check = expect_bool_eq(all5_bool(true, true, true, true, true), true)
    let any3_check = expect_bool_eq(any3_bool(false, false, true), true)
    let any4_check = expect_bool_eq(any4_bool(false, false, false, false), false)
    let any5_check = expect_bool_eq(any5_bool(false, false, false, false, true), true)
    let none3_check = expect_bool_eq(none3_bool(false, false, false), true)
    let none4_check = expect_bool_eq(none4_bool(false, false, true, false), false)
    let none5_check = expect_bool_eq(none5_bool(false, false, false, false, false), true)
    let bool_ne_check = expect_bool_ne(true, false)
    let bool_not_check = expect_bool_not(false, true)
    let bool_and_check = expect_bool_and(true, false, false)
    let bool_or_check = expect_bool_or(false, true, true)
    let bool_xor_check = expect_bool_xor(true, true, false)
    let core_implies_check = expect_bool_eq(implies_bool(true, false), false)
    let core_ascending_check = expect_bool_eq(is_ascending_int(20, 21, 22), true)
    let core_ascending4_check = expect_bool_eq(is_ascending4_int(20, 21, 21, 22), true)
    let core_ascending5_check = expect_bool_eq(is_ascending5_int(20, 21, 21, 22, 23), true)
    let core_strict_ascending_check = expect_bool_eq(is_strictly_ascending_int(20, 20, 22), false)
    let core_strict_ascending4_check = expect_bool_eq(is_strictly_ascending4_int(20, 21, 22, 23), true)
    let core_strict_ascending5_check = expect_bool_eq(is_strictly_ascending5_int(20, 21, 22, 23, 24), true)
    let core_descending_check = expect_bool_eq(is_descending_int(22, 21, 20), true)
    let core_descending4_check = expect_bool_eq(is_descending4_int(22, 21, 21, 20), true)
    let core_descending5_check = expect_bool_eq(is_descending5_int(23, 22, 21, 21, 20), true)
    let core_strict_descending_check = expect_bool_eq(is_strictly_descending_int(22, 22, 20), false)
    let core_strict_descending4_check = expect_bool_eq(is_strictly_descending4_int(23, 22, 21, 20), true)
    let core_strict_descending5_check = expect_bool_eq(is_strictly_descending5_int(24, 23, 22, 21, 20), true)
    let core_bounds_check = expect_bool_eq(in_bounds_int(21, 22, 20), true)
    let core_exclusive_bounds_check = expect_bool_eq(in_exclusive_bounds_int(22, 22, 20), false)
    let core_within_check = expect_bool_eq(is_within_int(21, 22, 1), true)
    let core_not_within_check = expect_bool_eq(is_not_within_int(19, 22, 1), true)
    let core_outside_range_check = expect_bool_eq(is_outside_range_int(19, 20, 22), true)
    let core_outside_bounds_check = expect_bool_eq(is_outside_bounds_int(19, 22, 20), true)
    let range_check = expect_int_between(22, 20, 22)
    let exclusive_range_check = expect_int_exclusive_between(21, 20, 22)
    let outside_check = expect_int_outside(19, 20, 22)
    let bounds_check = expect_int_between_bounds(21, 22, 20)
    let exclusive_bounds_check = expect_int_exclusive_between_bounds(21, 22, 20)
    let outside_bounds_check = expect_int_outside_bounds(19, 22, 20)
    let clamp_min_expect_check = expect_int_clamp_min(19, 20, 20)
    let clamp_max_expect_check = expect_int_clamp_max(23, 22, 22)
    let clamped_check = expect_int_clamped(19, 20, 22, 20)
    let clamped_bounds_check = expect_int_clamped_bounds(23, 22, 20, 22)
    let distance_range_expect_check = expect_int_distance_to_range(19, 20, 22, 1)
    let distance_bounds_expect_check = expect_int_distance_to_bounds(23, 22, 20, 1)
    let bool_all3_check = expect_bool_all3(true, true, true, true)
    let bool_all4_check = expect_bool_all4(true, true, false, true, false)
    let bool_all5_check = expect_bool_all5(true, true, true, true, true, true)
    let bool_any3_check = expect_bool_any3(false, false, true, true)
    let bool_any4_check = expect_bool_any4(false, false, false, false, false)
    let bool_any5_check = expect_bool_any5(false, false, false, false, true, true)
    let bool_none3_check = expect_bool_none3(false, false, false, true)
    let bool_none4_check = expect_bool_none4(false, false, true, false, false)
    let bool_none5_check = expect_bool_none5(false, false, false, false, false, true)
    let bool_to_int_expect_check = expect_bool_to_int(true, 1)
    let array_sum_check = expect_int_array_sum3([2, 3, 4], 9)
    let array_sum5_check = expect_int_array_sum5([2, 3, 4, 5, 6], 20)
    let array_product_check = expect_int_array_product4([2, 3, 4, 5], 120)
    let array_extrema_check = expect_int_array_max5([3, 9, 5, 7, 11], 11) + expect_int_array_min5([3, 9, 5, 7, 1], 1)
    let array_bool_check = expect_bool_array_all3([true, true, true], true) + expect_bool_array_any5([false, false, false, false, true], true) + expect_bool_array_none4([false, false, false, false], true)
    let array_generic_check = expect_int_array_first3([8, 9, 10], 8) + expect_bool_array_last5([true, false, true, false, true], true)
    let array_at_check = expect_int_array_at3([8, 9, 10], 1, 0, 9) + expect_bool_array_at5([true, false, true, false, true], 8, false, false)
    let array_transform_check = expect_int_array_reverse3([8, 9, 10], 10, 8) + expect_int_array_repeat3(7, 7) + expect_bool_array_repeat5(false, false)
    let array_query_check = expect_int_array_contains3([8, 9, 10], 9, true) + expect_int_array_count3([8, 9, 8], 8, 2) + expect_bool_array_contains5([true, false, true, false, true], false, true) + expect_bool_array_count5([true, false, true, false, true], true, 3)
    let max_expect_check = expect_int_max(20, 22, 22)
    let min_expect_check = expect_int_min(20, 22, 20)
    let max3_expect_check = expect_int_max3(20, 22, 21, 22)
    let min3_expect_check = expect_int_min3(20, 22, 21, 20)
    let max4_expect_check = expect_int_max4(20, 22, 21, 19, 22)
    let min4_expect_check = expect_int_min4(20, 22, 21, 19, 19)
    let max5_expect_check = expect_int_max5(20, 22, 21, 19, 23, 23)
    let min5_expect_check = expect_int_min5(20, 22, 21, 19, 18, 18)
    let median3_expect_check = expect_int_median3(22, 20, 21, 21)
    let sum3_expect_check = expect_int_sum3(2, 3, 4, 9)
    let sum4_expect_check = expect_int_sum4(2, 3, 4, 5, 14)
    let sum5_expect_check = expect_int_sum5(2, 3, 4, 5, 6, 20)
    let product3_expect_check = expect_int_product3(2, 3, 4, 24)
    let product4_expect_check = expect_int_product4(2, 3, 4, 5, 120)
    let product5_expect_check = expect_int_product5(2, 3, 4, 5, 6, 720)
    let average2_expect_check = expect_int_average2(5, 8, 6)
    let average3_expect_check = expect_int_average3(3, 6, 9, 6)
    let average4_expect_check = expect_int_average4(2, 4, 6, 8, 5)
    let average5_expect_check = expect_int_average5(2, 4, 6, 8, 10, 6)
    let sign_expect_check = expect_int_sign(0 - 5, 0 - 1)
    let sign_zero_expect_check = expect_int_sign(0, 0)
    let compare_less_expect_check = expect_int_compare(3, 9, 0 - 1)
    let compare_equal_expect_check = expect_int_compare(9, 9, 0)
    let compare_greater_expect_check = expect_int_compare(9, 3, 1)
    let abs_expect_check = expect_int_abs(0 - 22, 22)
    let abs_diff_expect_check = expect_int_abs_diff(22, 19, 3)
    let range_span_expect_check = expect_int_range_span(22, 20, 2)
    let lower_bound_expect_check = expect_int_lower_bound(22, 20, 20)
    let upper_bound_expect_check = expect_int_upper_bound(22, 20, 22)
    let quotient_expect_check = expect_int_quotient_or_zero(21, 7, 3)
    let quotient_zero_expect_check = expect_int_quotient_or_zero(21, 0, 0)
    let remainder_expect_check = expect_int_remainder_or_zero(22, 7, 1)
    let remainder_zero_expect_check = expect_int_remainder_or_zero(22, 0, 0)
    let has_remainder_expect_check = expect_int_has_remainder(22, 7)
    let factor_expect_check = expect_int_factor_of(7, 21)
    let ascending_check = expect_int_ascending(20, 21, 22)
    let ascending4_check = expect_int_ascending4(20, 21, 21, 22)
    let ascending5_check = expect_int_ascending5(20, 21, 21, 22, 23)
    let strict_ascending_check = expect_int_strictly_ascending(20, 21, 22)
    let strict_ascending4_check = expect_int_strictly_ascending4(20, 21, 22, 23)
    let strict_ascending5_check = expect_int_strictly_ascending5(20, 21, 22, 23, 24)
    let descending_check = expect_int_descending(22, 21, 20)
    let descending4_check = expect_int_descending4(22, 21, 21, 20)
    let descending5_check = expect_int_descending5(23, 22, 21, 21, 20)
    let strict_descending_check = expect_int_strictly_descending(22, 21, 20)
    let strict_descending4_check = expect_int_strictly_descending4(23, 22, 21, 20)
    let strict_descending5_check = expect_int_strictly_descending5(24, 23, 22, 21, 20)
    let divisible_check = expect_int_divisible_by(21, 7)
    let within_check = expect_int_within(21, 22, 1)
    let not_within_check = expect_int_not_within(19, 22, 1)
    let even_check = expect_int_even(22)
    let odd_check = expect_int_odd(21)
    let positive_check = expect_int_positive(22)
    let negative_check = expect_int_negative(0 - 1)
    let nonnegative_check = expect_int_nonnegative(0)
    let nonpositive_check = expect_int_nonpositive(0)
    let test_implies_check = expect_bool_implies(false, false)
    let true_check = expect_true(true)
    let status_ok_bool_check = expect_bool_eq(is_status_ok(0), true)
    let status_failed_bool_check = expect_bool_eq(is_status_failed(1), true)
    let merged_status_check = expect_int_eq(merge_status(0, 1), 1)
    let merged_status3_check = expect_int_eq(merge_status3(0, 1, 1), 2)
    let merged_status4_check = expect_int_eq(merge_status4(0, 1, 1, 1), 3)
    let merged_status5_check = expect_int_eq(merge_status5(0, 1, 1, 1, 1), 4)
    let merged_status6_check = expect_int_eq(merge_status6(0, 1, 1, 1, 1, 1), 5)
    let status_ok_check = expect_status_ok(merge_status(0, 0))
    let status_failed_check = expect_status_failed(merge_status(0, 1))
    let failed_status_ok_check = expect_int_eq(expect_status_ok(1), 1)
    let failed_status_failed_check = expect_int_eq(expect_status_failed(0), 1)
    let failed_bool_ne_check = expect_int_eq(expect_bool_ne(true, true), 1)
    let failed_bool_not_check = expect_int_eq(expect_bool_not(false, false), 1)
    let failed_bool_and_check = expect_int_eq(expect_bool_and(true, false, true), 1)
    let failed_bool_or_check = expect_int_eq(expect_bool_or(false, false, true), 1)
    let failed_bool_xor_check = expect_int_eq(expect_bool_xor(true, false, false), 1)
    let failed_bool_all3_check = expect_int_eq(expect_bool_all3(true, true, false, true), 1)
    let failed_bool_all4_check = expect_int_eq(expect_bool_all4(true, true, true, true, false), 1)
    let failed_bool_all5_check = expect_int_eq(expect_bool_all5(true, true, true, true, false, true), 1)
    let failed_bool_any3_check = expect_int_eq(expect_bool_any3(false, false, false, true), 1)
    let failed_bool_any4_check = expect_int_eq(expect_bool_any4(false, false, true, false, false), 1)
    let failed_bool_any5_check = expect_int_eq(expect_bool_any5(false, false, false, false, false, true), 1)
    let failed_bool_none3_check = expect_int_eq(expect_bool_none3(false, true, false, true), 1)
    let failed_bool_none4_check = expect_int_eq(expect_bool_none4(false, false, false, false, false), 1)
    let failed_bool_none5_check = expect_int_eq(expect_bool_none5(false, false, true, false, false, true), 1)
    let failed_bool_to_int_check = expect_int_eq(expect_bool_to_int(false, 1), 1)
    let failed_range_check = expect_int_eq(expect_int_between(19, 20, 22), 1)
    let failed_exclusive_range_check = expect_int_eq(expect_int_exclusive_between(20, 20, 22), 1)
    let failed_outside_check = expect_int_eq(expect_int_outside(21, 20, 22), 1)
    let failed_bounds_check = expect_int_eq(expect_int_between_bounds(19, 22, 20), 1)
    let failed_exclusive_bounds_check = expect_int_eq(expect_int_exclusive_between_bounds(22, 22, 20), 1)
    let failed_outside_bounds_check = expect_int_eq(expect_int_outside_bounds(21, 22, 20), 1)
    let failed_clamp_min_check = expect_int_eq(expect_int_clamp_min(19, 20, 19), 1)
    let failed_clamp_max_check = expect_int_eq(expect_int_clamp_max(23, 22, 23), 1)
    let failed_clamped_check = expect_int_eq(expect_int_clamped(19, 20, 22, 19), 1)
    let failed_clamped_bounds_check = expect_int_eq(expect_int_clamped_bounds(23, 22, 20, 23), 1)
    let failed_distance_range_check = expect_int_eq(expect_int_distance_to_range(21, 20, 22, 1), 1)
    let failed_distance_bounds_check = expect_int_eq(expect_int_distance_to_bounds(21, 22, 20, 1), 1)
    let failed_max_check = expect_int_eq(expect_int_max(20, 22, 20), 1)
    let failed_min_check = expect_int_eq(expect_int_min(20, 22, 22), 1)
    let failed_max3_check = expect_int_eq(expect_int_max3(20, 22, 21, 21), 1)
    let failed_min3_check = expect_int_eq(expect_int_min3(20, 22, 21, 21), 1)
    let failed_max4_check = expect_int_eq(expect_int_max4(20, 22, 21, 19, 21), 1)
    let failed_min4_check = expect_int_eq(expect_int_min4(20, 22, 21, 19, 20), 1)
    let failed_max5_check = expect_int_eq(expect_int_max5(20, 22, 21, 19, 23, 22), 1)
    let failed_min5_check = expect_int_eq(expect_int_min5(20, 22, 21, 19, 18, 19), 1)
    let failed_median3_check = expect_int_eq(expect_int_median3(22, 20, 21, 22), 1)
    let failed_sum3_check = expect_int_eq(expect_int_sum3(2, 3, 4, 10), 1)
    let failed_sum4_check = expect_int_eq(expect_int_sum4(2, 3, 4, 5, 15), 1)
    let failed_sum5_check = expect_int_eq(expect_int_sum5(2, 3, 4, 5, 6, 21), 1)
    let failed_product3_check = expect_int_eq(expect_int_product3(2, 3, 4, 25), 1)
    let failed_product4_check = expect_int_eq(expect_int_product4(2, 3, 4, 5, 121), 1)
    let failed_product5_check = expect_int_eq(expect_int_product5(2, 3, 4, 5, 6, 721), 1)
    let failed_average2_check = expect_int_eq(expect_int_average2(5, 8, 7), 1)
    let failed_average3_check = expect_int_eq(expect_int_average3(3, 6, 9, 7), 1)
    let failed_average4_check = expect_int_eq(expect_int_average4(2, 4, 6, 8, 6), 1)
    let failed_average5_check = expect_int_eq(expect_int_average5(2, 4, 6, 8, 10, 7), 1)
    let failed_sign_check = expect_int_eq(expect_int_sign(5, 0 - 1), 1)
    let failed_compare_equal_check = expect_int_eq(expect_int_compare(9, 9, 1), 1)
    let failed_compare_order_check = expect_int_eq(expect_int_compare(3, 9, 1), 1)
    let failed_abs_check = expect_int_eq(expect_int_abs(0 - 22, 0 - 22), 1)
    let failed_abs_diff_check = expect_int_eq(expect_int_abs_diff(22, 19, 2), 1)
    let failed_range_span_check = expect_int_eq(expect_int_range_span(22, 20, 3), 1)
    let failed_lower_bound_check = expect_int_eq(expect_int_lower_bound(22, 20, 22), 1)
    let failed_upper_bound_check = expect_int_eq(expect_int_upper_bound(22, 20, 20), 1)
    let failed_quotient_check = expect_int_eq(expect_int_quotient_or_zero(21, 7, 4), 1)
    let failed_quotient_zero_check = expect_int_eq(expect_int_quotient_or_zero(21, 0, 1), 1)
    let failed_remainder_check = expect_int_eq(expect_int_remainder_or_zero(22, 7, 2), 1)
    let failed_remainder_zero_check = expect_int_eq(expect_int_remainder_or_zero(22, 0, 1), 1)
    let failed_has_remainder_check = expect_int_eq(expect_int_has_remainder(21, 7), 1)
    let failed_factor_check = expect_int_eq(expect_int_factor_of(0, 21), 1)
    let failed_ascending_check = expect_int_eq(expect_int_ascending(22, 21, 20), 1)
    let failed_ascending4_check = expect_int_eq(expect_int_ascending4(20, 22, 21, 23), 1)
    let failed_ascending5_check = expect_int_eq(expect_int_ascending5(20, 21, 23, 22, 24), 1)
    let failed_strict_ascending_check = expect_int_eq(expect_int_strictly_ascending(20, 20, 22), 1)
    let failed_strict_ascending4_check = expect_int_eq(expect_int_strictly_ascending4(20, 21, 21, 23), 1)
    let failed_strict_ascending5_check = expect_int_eq(expect_int_strictly_ascending5(20, 21, 22, 23, 23), 1)
    let failed_descending_check = expect_int_eq(expect_int_descending(20, 22, 21), 1)
    let failed_descending4_check = expect_int_eq(expect_int_descending4(22, 20, 21, 19), 1)
    let failed_descending5_check = expect_int_eq(expect_int_descending5(23, 22, 20, 21, 19), 1)
    let failed_strict_descending_check = expect_int_eq(expect_int_strictly_descending(22, 22, 20), 1)
    let failed_strict_descending4_check = expect_int_eq(expect_int_strictly_descending4(23, 22, 22, 20), 1)
    let failed_strict_descending5_check = expect_int_eq(expect_int_strictly_descending5(24, 23, 22, 22, 20), 1)
    let failed_divisible_check = expect_int_eq(expect_int_divisible_by(21, 0), 1)
    let failed_within_check = expect_int_eq(expect_int_within(19, 22, 1), 1)
    let failed_not_within_check = expect_int_eq(expect_int_not_within(22, 22, 0), 1)
    let failed_even_check = expect_int_eq(expect_int_even(21), 1)
    let failed_odd_check = expect_int_eq(expect_int_odd(22), 1)
    let failed_positive_check = expect_int_eq(expect_int_positive(0), 1)
    let failed_negative_check = expect_int_eq(expect_int_negative(0), 1)
    let failed_nonnegative_check = expect_int_eq(expect_int_nonnegative(0 - 1), 1)
    let failed_nonpositive_check = expect_int_eq(expect_int_nonpositive(1), 1)
    let failed_implies_check = expect_int_eq(expect_bool_implies(true, false), 1)
    let option_status = merge_status4(expect_int_option_some(some_int(22), 22), expect_int_option_none(none_int()), expect_bool_option_some(some_bool(true), true), expect_bool_option_none(none_bool()))
    let option_or_status = merge_status4(expect_int_option_or(none_int(), some_int(9), 9), expect_bool_option_or(none_bool(), some_bool(false), false), 0, 0)
    let result_status = merge_status4(expect_int_result_ok(result_ok_int(22), 22), expect_int_result_err(result_err_int(7), 7), expect_bool_result_ok(result_ok_bool(true), true), expect_bool_result_err(result_err_bool(3), 3))
    let result_or_status = merge_status4(expect_int_result_or(result_err_int(5), result_ok_int(9), 9), expect_bool_result_or(result_err_bool(6), result_ok_bool(false), false), 0, 0)
    let conversion_status = merge_status4(expect_int_result_to_option_some(result_ok_int(31), 31), expect_int_result_to_option_none(result_err_int(8)), expect_int_option_ok_or(some_int(31), 8, 31), expect_int_option_ok_or_err(none_int(), 8))
    let conversion_bool_status = merge_status4(expect_bool_result_to_option_some(result_ok_bool(false), false), expect_bool_result_to_option_none(result_err_bool(9)), expect_bool_option_ok_or(some_bool(true), 9, true), expect_bool_option_ok_or_err(none_bool(), 9))
    let error_option_status = merge_status4(expect_int_result_error_some(result_err_int(0), 0), expect_int_result_error_none(result_ok_int(31)), expect_bool_result_error_some(result_err_bool(0), 0), expect_bool_result_error_none(result_ok_bool(false)))
    let generic_none_int: Option[Int] = option_none()
    let generic_option_status = merge_status4(expect_generic_int_option_some(Option.Some(7), 7), expect_generic_int_option_none(generic_none_int), expect_generic_bool_option_some(Option.Some(true), true), expect_generic_bool_option_none(Option.None))
    let generic_option_or_status = merge_status4(expect_generic_int_option_or(generic_none_int, Option.Some(9), 9), expect_generic_bool_option_or(Option.None, Option.Some(false), false), 0, 0)
    let generic_result_status = merge_status4(expect_generic_int_result_ok(Result.Ok(7), 7), expect_generic_int_result_err(Result.Err(3), 3), expect_generic_bool_result_ok(Result.Ok(true), true), expect_generic_bool_result_err(Result.Err(4), 4))
    let generic_result_or_status = merge_status4(expect_generic_int_result_or(Result.Err(5), Result.Ok(11), 11), expect_generic_bool_result_or(Result.Err(6), Result.Ok(false), false), 0, 0)
    let generic_result_error_status = merge_status4(expect_generic_int_result_error(Result.Err(8), 0, 8), expect_generic_int_result_error(Result.Ok(14), 0, 0), expect_generic_bool_result_error(Result.Err(9), 0, 9), expect_generic_bool_result_error(Result.Ok(false), 0, 0))
    let generic_none_bool: Option[Bool] = option_none()
    let generic_result_conversion_status = merge_status4(expect_generic_int_result_to_option_some(Result.Ok(31), 31), expect_generic_int_result_to_option_none(Result.Err(8)), expect_generic_bool_result_to_option_some(Result.Ok(false), false), expect_generic_bool_result_to_option_none(Result.Err(9)))
    let generic_result_error_option_status = merge_status4(expect_generic_int_result_error_some(Result.Err(0), 0), expect_generic_int_result_error_none(Result.Ok(31)), expect_generic_bool_result_error_some(Result.Err(0), 0), expect_generic_bool_result_error_none(Result.Ok(false)))
    let generic_option_conversion_status = merge_status4(expect_generic_int_option_ok_or(Option.Some(31), 8, 31), expect_generic_int_option_ok_or_err(generic_none_int, 8), expect_generic_bool_option_ok_or(Option.Some(true), 9, true), expect_generic_bool_option_ok_or_err(generic_none_bool, 9))

    let core_status = merge_status6(max_check + max3_check + max4_check + max5_check + min_check, min3_check + min4_check + min5_check + median3_check + sum3_check, sum4_check + sum5_check + product3_check + product4_check, product5_check + average2_check + average3_check + average4_check + average5_check + quotient_check, quotient_zero_check + remainder_check + remainder_zero_check + has_remainder_check + factor_check + clamp_min_check, clamp_max_check + clamp_bounds_check + abs_check + abs_diff_check + range_span_check + compare_check + sign_negative_check + sign_zero_check + sign_positive_check + and_check + xor_check + all3_check + all4_check + all5_check + any3_check + any4_check + any5_check + none3_check + none4_check + none5_check + bool_ne_check + bool_not_check + bool_and_check + bool_or_check + core_descending_check + core_descending4_check + core_descending5_check + core_strict_descending_check + core_strict_descending4_check + core_strict_descending5_check + core_not_within_check + core_outside_range_check + core_outside_bounds_check + lower_bound_check + upper_bound_check + distance_range_check + distance_bounds_check)
    let bool_status = merge_status4(bool_xor_check + core_implies_check + core_ascending_check + core_ascending4_check + core_ascending5_check + core_strict_ascending_check + core_strict_ascending4_check + core_strict_ascending5_check, core_bounds_check + core_exclusive_bounds_check + core_within_check + range_check, bool_all3_check + bool_all4_check + bool_all5_check + bool_any3_check + bool_any4_check + bool_any5_check, bool_none3_check + bool_none4_check + bool_none5_check + bool_to_int_expect_check + failed_bool_ne_check + failed_bool_not_check + failed_bool_and_check + failed_bool_or_check + failed_bool_xor_check + failed_bool_all3_check + failed_bool_all4_check + failed_bool_all5_check + failed_bool_any3_check + failed_bool_any4_check + failed_bool_any5_check + failed_bool_none3_check + failed_bool_none4_check + failed_bool_none5_check + failed_bool_to_int_check + exclusive_range_check + outside_check + bounds_check)
    let range_status = merge_status5(exclusive_bounds_check + outside_bounds_check + clamp_min_expect_check + clamp_max_expect_check + clamped_check + clamped_bounds_check, distance_range_expect_check + distance_bounds_expect_check + max_expect_check + min_expect_check, max3_expect_check + min3_expect_check + max4_expect_check + min4_expect_check + max5_expect_check + min5_expect_check + median3_expect_check, sum3_expect_check + sum4_expect_check + sum5_expect_check + product3_expect_check + product4_expect_check + product5_expect_check + average2_expect_check + average3_expect_check + average4_expect_check + average5_expect_check, sign_expect_check + sign_zero_expect_check + compare_less_expect_check + compare_equal_expect_check + compare_greater_expect_check + abs_expect_check + abs_diff_expect_check + range_span_expect_check + lower_bound_expect_check + upper_bound_expect_check + quotient_expect_check + quotient_zero_expect_check + remainder_expect_check + remainder_zero_expect_check + has_remainder_expect_check + factor_expect_check + ascending_check + ascending4_check + ascending5_check + strict_ascending_check + strict_ascending4_check + strict_ascending5_check + descending_check + descending4_check + descending5_check + strict_descending_check + strict_descending4_check + strict_descending5_check + divisible_check + within_check + not_within_check + even_check + odd_check + positive_check + negative_check + nonnegative_check + nonpositive_check + test_implies_check + true_check + status_ok_bool_check)
    let status_helper_status = merge_status4(status_failed_bool_check + merged_status_check + merged_status3_check + merged_status4_check, merged_status5_check + merged_status6_check + status_ok_check + status_failed_check, failed_status_ok_check + failed_status_failed_check + failed_range_check + failed_exclusive_range_check, failed_outside_check + failed_bounds_check + failed_exclusive_bounds_check + failed_outside_bounds_check)
    let failure_status = merge_status4(failed_clamp_min_check + failed_clamp_max_check + failed_clamped_check + failed_clamped_bounds_check + failed_distance_range_check + failed_distance_bounds_check, failed_max_check + failed_min_check + failed_max3_check + failed_min3_check + failed_max4_check + failed_min4_check + failed_max5_check + failed_min5_check + failed_median3_check, failed_sum3_check + failed_sum4_check + failed_sum5_check + failed_product3_check + failed_product4_check + failed_product5_check + failed_average2_check + failed_average3_check + failed_average4_check + failed_average5_check + failed_sign_check + failed_compare_equal_check + failed_compare_order_check + failed_abs_check + failed_abs_diff_check + failed_range_span_check + failed_lower_bound_check + failed_upper_bound_check + failed_quotient_check + failed_quotient_zero_check, failed_remainder_check + failed_remainder_zero_check + failed_has_remainder_check + failed_factor_check + failed_ascending_check + failed_ascending4_check + failed_ascending5_check + failed_strict_ascending_check + failed_strict_ascending4_check + failed_strict_ascending5_check + failed_descending_check + failed_descending4_check + failed_descending5_check + failed_strict_descending_check + failed_strict_descending4_check + failed_strict_descending5_check + failed_divisible_check + failed_within_check + failed_not_within_check + failed_even_check + failed_odd_check + failed_positive_check + failed_negative_check + failed_nonnegative_check + failed_nonpositive_check + failed_implies_check)
    let array_status = merge_status6(array_sum_check, array_sum5_check, array_product_check, array_extrema_check, array_bool_check, array_generic_check + array_at_check + array_transform_check + array_query_check)

    return expect_status_ok(merge_status6(core_status, bool_status, range_status, status_helper_status, failure_status, merge_status6(array_status + option_status, option_or_status, result_status, result_or_status, conversion_status + generic_option_status + generic_option_or_status + generic_result_status + generic_result_conversion_status, conversion_bool_status + error_option_status + generic_result_or_status + generic_result_error_status + generic_result_error_option_status + generic_option_conversion_status)))
}
"#
}

fn resolve_project_workspace_member_command_request_root(path: &Path) -> Option<PathBuf> {
    if !path.is_dir()
        && !is_ql_source_file(path)
        && !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    Some(resolve_project_member_request_root(&manifest.manifest_path))
}

fn load_workspace_build_targets_for_command_from_request_root(
    _request_path: &Path,
    request_root: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(request_root).map_err(|error| {
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

fn project_emit_interface_path(
    path: &Path,
    output: Option<&Path>,
    selected_package_name: Option<&str>,
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
    let command_label = if check_only {
        check_command_label.as_str()
    } else {
        emit_command_label.as_str()
    };
    if let Some(package_name) = selected_package_name
        && let Err(message) = validate_project_package_name(package_name)
    {
        eprintln!("error: {command_label} {message}");
        return Err(1);
    }
    let request_root = if output.is_none() {
        resolve_project_workspace_member_command_request_root(path)
    } else {
        None
    };
    let manifest = load_project_manifest(request_root.as_deref().unwrap_or(path)).map_err(|error| {
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

    if output.is_some() && manifest.workspace.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let (_, package_manifest) = resolve_project_selected_package_manifest(
                path,
                Some(selected_package_name),
                emit_command_label.as_str(),
            )?;
            match emit_package_interface_path(
                &package_manifest.manifest_path,
                output,
                emit_command_label.as_str(),
                changed_only,
            ) {
                Ok(result) => report_emit_interface_result(result),
                Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
                    report_package_interface_failure(
                        &package_manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                    return Err(1);
                }
                Err(EmitPackageInterfaceError::SourceFailure { code, .. }) => {
                    report_package_interface_source_failure(
                        &package_manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                    return Err(code);
                }
                Err(EmitPackageInterfaceError::Code { code, .. }) => {
                    report_package_interface_failure(
                        &package_manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                    return Err(code);
                }
                Err(EmitPackageInterfaceError::ManifestFailure { .. }) => {
                    report_package_interface_manifest_failure(
                        &package_manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                    return Err(1);
                }
                Err(EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. }) => {
                    report_package_interface_no_sources_failure(
                        &package_manifest.manifest_path,
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
                        &package_manifest.manifest_path,
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
                        &package_manifest.manifest_path,
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
    }

    if manifest.package.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let actual_package_name = package_name(&manifest).map_err(|error| {
                eprintln!("error: {command_label} {error}");
                if check_only {
                    report_package_interface_check_manifest_failure(
                        &manifest.manifest_path,
                        changed_only,
                    );
                } else {
                    report_package_interface_manifest_failure(
                        &manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                }
                1
            })?;
            if actual_package_name != selected_package_name {
                eprintln!(
                    "error: {command_label} package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                    normalize_path(path)
                );
                return Err(1);
            }
        }
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

    let Some(_) = &manifest.workspace else {
        eprintln!("error: `ql project emit-interface` requires `[package]` or `[workspace]`");
        return Err(1);
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let selected_members =
        select_workspace_members(&manifest, path, selected_package_name, command_label)?;
    let mut failing_member_count = 0usize;
    let mut emission_failure_count = 0usize;
    let mut first_failing_member_manifest = None;
    for member in &selected_members {
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

fn is_version_command(command: &str) -> bool {
    matches!(command, "--version" | "-V" | "version")
}

fn version_text(binary_name: &str) -> String {
    format!("{binary_name} {CLI_VERSION}")
}

fn print_usage() {
    eprintln!("Qlang CLI {}", CLI_VERSION);
    eprintln!("usage:");
    eprintln!("  ql --version");
    eprintln!("  ql version");
    eprintln!("  ql check <file-or-dir> [--sync-interfaces] [--json]");
    eprintln!(
        "  ql build <file-or-dir> [--emit llvm-ir|asm|obj|exe|dylib|staticlib] [--profile debug|release|--release] [--package <name>] [--lib|--bin <name>|--target <path>] [--list] [-o <output>] [--emit-interface] [--header] [--header-surface exports|imports|both] [--header-output <output>] [--json]"
    );
    eprintln!(
        "  ql run <file-or-dir> [--profile debug|release|--release] [--package <name>] [--bin <name>|--target <path>] [--list] [--json] [-- <args...>]"
    );
    eprintln!(
        "  ql test <file-or-dir> [--profile debug|release|--release] [--package <name>] [--target <tests/...ql>] [--list] [--filter <substring>] [--json]"
    );
    eprintln!(
        "  ql project targets [file-or-dir] [--package <name>] [--lib|--bin <name>|--target <path>] [--json]"
    );
    eprintln!("  ql project status [file-or-dir] [--package <name>] [--json]");
    eprintln!("  ql project target add [file-or-dir] [--package <name>] --bin <name>");
    eprintln!("  ql project graph [file-or-dir] [--package <name>] [--json]");
    eprintln!("  ql project dependents [file-or-dir] [--name <package>] [--json]");
    eprintln!("  ql project dependencies [file-or-dir] [--name <package>] [--json]");
    eprintln!("  ql project lock [file-or-dir] [--check] [--json]");
    eprintln!("  ql project init [dir] [--workspace] [--name <package>] [--stdlib <path>]");
    eprintln!("  ql project add [file-or-dir] --name <package> [--dependency <package> ...]");
    eprintln!("  ql project add [file-or-dir] --existing <file-or-dir>");
    eprintln!("  ql project remove [file-or-dir] --name <package> [--cascade]");
    eprintln!(
        "  ql project add-dependency [file-or-dir] [--package <name>] (--name <package> | --path <file-or-dir>)"
    );
    eprintln!(
        "  ql project remove-dependency [file-or-dir] [--package <name>] [--name <package>] [--all]"
    );
    eprintln!(
        "  ql project emit-interface [file-or-dir] [--package <name>] [-o <output>] [--changed-only] [--check]"
    );
    eprintln!("  ql ffi header <file> [--surface exports|imports|both] [-o <output>]");
    eprintln!("  ql fmt <file> [--write]");
    eprintln!("  ql mir <file>");
    eprintln!("  ql ownership <file>");
    eprintln!("  ql runtime <file>");
}

#[cfg(test)]
mod tests;
