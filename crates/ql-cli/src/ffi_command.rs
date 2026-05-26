use std::path::{Path, PathBuf};

use ql_driver::{
    BuildError, CHeaderError, CHeaderOptions, CHeaderSurface, acquire_build_output_locks,
    emit_c_header, resolve_c_header_output_path,
};

use crate::cli_diagnostics::print_diagnostics;
use crate::cli_usage::print_usage;
use crate::cli_utils::normalize_path;

pub(crate) fn ffi_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
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

            let options = parse_c_header_options(args)?;
            emit_c_header_path(Path::new(&path), &options)
        }
        other => {
            eprintln!("error: unknown `ql ffi` subcommand `{other}`");
            print_usage();
            Err(1)
        }
    }
}

fn parse_c_header_options(args: &mut impl Iterator<Item = String>) -> Result<CHeaderOptions, u8> {
    let mut options = CHeaderOptions::default();
    let remaining = args.collect::<Vec<_>>();
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "-o" | "--output" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql ffi header --output` expects a file path");
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
                    eprintln!("error: unsupported `ql ffi header` surface `{value}`");
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

    Ok(options)
}

fn emit_c_header_path(path: &Path, options: &CHeaderOptions) -> Result<(), u8> {
    let output_path = match resolve_c_header_output_path(path, options) {
        Ok(path) => path,
        Err(error) => return report_c_header_error(error),
    };
    let _output_lock = acquire_build_output_locks(vec![output_path])
        .map_err(c_header_output_lock_error_message)
        .map_err(|message| {
            eprintln!("error: {message}");
            1
        })?;

    match emit_c_header(path, options) {
        Ok(artifact) => {
            println!("wrote c-header: {}", artifact.path.display());
            Ok(())
        }
        Err(error) => report_c_header_error(error),
    }
}

fn report_c_header_error(error: CHeaderError) -> Result<(), u8> {
    match error {
        CHeaderError::InvalidInput(message) => {
            eprintln!("error: {message}");
            Err(1)
        }
        CHeaderError::Io { path, error } => {
            eprintln!("error: failed to access `{}`: {error}", path.display());
            Err(1)
        }
        CHeaderError::Diagnostics {
            path,
            source,
            diagnostics,
        } => {
            print_diagnostics(&path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn c_header_output_lock_error_message(error: BuildError) -> String {
    match error {
        BuildError::Io { path, error } => format!(
            "failed to acquire c-header output lock `{}`: {error}",
            normalize_path(&path)
        ),
        BuildError::InvalidInput(message) => message,
        BuildError::Diagnostics { path, .. } => format!(
            "failed to acquire c-header output lock while diagnostics were reported for `{}`",
            normalize_path(&path)
        ),
        BuildError::Toolchain { error, .. } => format!("{error}"),
    }
}
