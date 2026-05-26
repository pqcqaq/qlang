use std::fs;
use std::path::Path;

use ql_analysis::parse_errors_to_diagnostics;
use ql_driver::{BuildError, acquire_build_output_locks, write_file_atomically};
use ql_fmt::format_source;

use crate::cli_diagnostics::print_diagnostics;
use crate::cli_utils::normalize_path;

pub(crate) fn fmt_path(args: impl Iterator<Item = String>) -> Result<(), u8> {
    let (path, write) = parse_fmt_args(args)?;
    format_path(Path::new(&path), write)
}

fn parse_fmt_args(args: impl Iterator<Item = String>) -> Result<(String, bool), u8> {
    let mut write = false;
    let mut path = None;

    for arg in args {
        match arg.as_str() {
            "--write" => {
                write = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql fmt` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql fmt` argument `{other}`");
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }
    }

    let Some(path) = path else {
        eprintln!("error: `ql fmt` expects a file path");
        return Err(1);
    };

    Ok((path, write))
}

pub(crate) fn format_path(path: &Path, write: bool) -> Result<(), u8> {
    let _source_lock = if write {
        Some(
            acquire_build_output_locks(vec![path.to_path_buf()])
                .map_err(format_source_lock_error_message)
                .map_err(|message| {
                    eprintln!(
                        "error: `ql fmt --write` failed to lock source `{}`: {message}",
                        normalize_path(path)
                    );
                    1
                })?,
        )
    } else {
        None
    };

    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match format_source(&source) {
        Ok(formatted) => {
            if write {
                write_file_atomically(path, &formatted).map_err(|error| {
                    eprintln!(
                        "error: failed to write formatted source `{}` atomically: {error}",
                        normalize_path(path)
                    );
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

fn format_source_lock_error_message(error: BuildError) -> String {
    match error {
        BuildError::Io { path, error } => format!(
            "failed to acquire source file lock `{}`: {error}",
            normalize_path(&path)
        ),
        BuildError::InvalidInput(message) => message,
        BuildError::Diagnostics { path, .. } => format!(
            "failed to acquire source file lock while diagnostics were reported for `{}`",
            normalize_path(&path)
        ),
        BuildError::Toolchain { error, .. } => format!("{error}"),
    }
}
