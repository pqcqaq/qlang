use std::path::{Path, PathBuf};

use ql_driver::{BuildCHeaderOptions, BuildEmit, BuildOptions, BuildProfile, CHeaderSurface};

use crate::cli_build_profile::{parse_cli_build_profile, set_cli_build_profile};
use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    list_build_targets_path, parse_project_target_selector_option,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
};
use crate::{
    BuildJsonReport, build_json_emit_interface_failure, build_json_preflight_failure,
    build_project_path, build_single_source_target, build_single_source_target_result,
    emit_built_package_interface, emit_built_package_interface_quiet,
};

pub(crate) fn build_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let Some(path) = args.next() else {
        eprintln!("error: `ql build` expects a file or directory path");
        return Err(1);
    };

    let options = parse_build_args(args)?;
    if options.list {
        return list_build_targets_path(Path::new(&path), &options.selector, options.json);
    }
    build_path(
        Path::new(&path),
        &options.build_options,
        &options.selector,
        options.emit_interface,
        options.emit_overridden,
        options.profile_overridden,
        options.json,
    )
}

pub(crate) fn build_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
) -> Result<(), u8> {
    match resolve_project_command_path(path, selector) {
        Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path,
            selector,
        }) => {
            return build_project_path(
                path,
                options,
                &selector,
                emit_interface,
                emit_overridden,
                profile_overridden,
                json,
                request_root_manifest_path.as_deref(),
            );
        }
        Ok(ResolvedProjectCommandPath::DirectSource) => {}
        Err(ProjectCommandPathError::SourcePathRejectsSelector) => {
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
                    "direct project source paths do not support target selectors".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_source_path_rejects_target_selector("`ql build`", path, selector);
            }
            return Err(1);
        }
        Err(ProjectCommandPathError::SelectorRequiresProjectContext) => {
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
            match emit_built_package_interface_quiet(path, path, options, &artifact.path, &[]) {
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
        emit_built_package_interface(path, path, options, &artifact.path, &[])?;
    }
    Ok(())
}

struct BuildCliOptions {
    build_options: BuildOptions,
    selector: ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
    list: bool,
}

fn parse_build_args(args: &mut impl Iterator<Item = String>) -> Result<BuildCliOptions, u8> {
    let mut build_options = BuildOptions::default();
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
                    "llvm-ir" => build_options.emit = BuildEmit::LlvmIr,
                    "asm" => build_options.emit = BuildEmit::Assembly,
                    "obj" => build_options.emit = BuildEmit::Object,
                    "exe" => build_options.emit = BuildEmit::Executable,
                    "dylib" => build_options.emit = BuildEmit::DynamicLibrary,
                    "staticlib" => build_options.emit = BuildEmit::StaticLibrary,
                    other => {
                        eprintln!("error: unsupported build emit target `{other}`");
                        return Err(1);
                    }
                }
            }
            "--release" => {
                set_cli_build_profile("`ql build`", &mut profile_override, BuildProfile::Release)?;
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
                build_options.output = Some(PathBuf::from(value));
            }
            "--header" => {
                build_options
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
                let header = build_options
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
                let header = build_options
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
        build_options.profile = profile;
    }

    Ok(BuildCliOptions {
        build_options,
        selector,
        emit_interface,
        emit_overridden,
        profile_overridden,
        json,
        list,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<BuildCliOptions, u8> {
        parse_build_args(&mut args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn parse_build_args_accepts_json_list_emit_interface_and_output() {
        let options = parse(&[
            "--json",
            "--list",
            "--emit-interface",
            "--emit",
            "obj",
            "--output",
            "out/app.o",
        ])
        .expect("build args should parse");

        assert!(options.json);
        assert!(options.list);
        assert!(options.emit_interface);
        assert!(options.emit_overridden);
        assert!(matches!(options.build_options.emit, BuildEmit::Object));
        assert_eq!(
            options.build_options.output.as_deref(),
            Some(Path::new("out/app.o"))
        );
    }

    #[test]
    fn parse_build_args_rejects_conflicting_profile_options() {
        assert!(parse(&["--release", "--profile", "debug"]).is_err());
    }
}
