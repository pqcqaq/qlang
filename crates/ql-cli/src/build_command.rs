use std::path::{Path, PathBuf};

use ql_driver::{BuildCHeaderOptions, BuildEmit, BuildOptions, BuildProfile, CHeaderSurface};

use crate::cli_build_profile::{parse_cli_build_profile, set_cli_build_profile};

use super::{
    ProjectTargetSelector, build_path, list_build_targets_path,
    parse_project_target_selector_option,
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
