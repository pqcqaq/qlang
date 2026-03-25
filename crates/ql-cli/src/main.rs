use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ql_fmt::format_source;
use ql_parser::{ParseError, parse_source};
use ql_span::{locate, slice_for_line};

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
            let Some(path) = args.next() else {
                eprintln!("error: `ql check` expects a file or directory path");
                return Err(1);
            };
            check_path(Path::new(&path))
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
        _ => {
            eprintln!("error: unknown command `{command}`");
            print_usage();
            Err(1)
        }
    }
}

fn check_path(path: &Path) -> Result<(), u8> {
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

        match parse_source(&source) {
            Ok(_) => {
                println!("ok: {}", file.display());
            }
            Err(errors) => {
                has_errors = true;
                print_errors(&file, &source, &errors);
            }
        }
    }

    if has_errors { Err(1) } else { Ok(()) }
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
            print_errors(path, &source, &errors);
            Err(1)
        }
    }
}

fn collect_ql_files(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    collect_ql_files_recursive(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ql_files_recursive(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_ql_files_recursive(&entry_path, files)?;
        } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("ql") {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn print_errors(path: &Path, source: &str, errors: &[ParseError]) {
    for error in errors {
        let location = locate(source, error.span);
        eprintln!(
            "error: {}:{}:{}: {}",
            path.display(),
            location.start.line,
            location.start.column,
            error.message
        );
        if let Some(line) = slice_for_line(source, location.start.line) {
            eprintln!("  {line}");
            let indent = " ".repeat(location.start.column.saturating_sub(1));
            let marker_len = error.span.len().max(1);
            eprintln!("  {indent}{}", "^".repeat(marker_len.min(8)));
        }
    }
}

fn print_usage() {
    eprintln!("Qlang CLI");
    eprintln!("usage:");
    eprintln!("  ql check <file-or-dir>");
    eprintln!("  ql fmt <file> [--write]");
}
