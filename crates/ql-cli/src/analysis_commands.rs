use std::fs;
use std::path::Path;

use ql_runtime::{collect_runtime_hook_signatures, collect_runtime_hooks};

use super::{analyze_semantics, print_diagnostics};

pub(crate) fn mir_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let path = single_path_argument("mir", args)?;
    render_mir_path(Path::new(&path))
}

pub(crate) fn ownership_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let path = single_path_argument("ownership", args)?;
    render_ownership_path(Path::new(&path))
}

pub(crate) fn runtime_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let path = single_path_argument("runtime", args)?;
    render_runtime_requirements_path(Path::new(&path))
}

fn single_path_argument(
    command_name: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<String, u8> {
    let Some(path) = args.next() else {
        eprintln!("error: `ql {command_name}` expects a file path");
        return Err(1);
    };
    if let Some(extra) = args.next() {
        eprintln!("error: unknown `ql {command_name}` argument `{extra}`");
        return Err(1);
    }
    Ok(path)
}

pub(crate) fn render_mir_path(path: &Path) -> Result<(), u8> {
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

pub(crate) fn render_ownership_path(path: &Path) -> Result<(), u8> {
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

pub(crate) fn render_runtime_requirements(analysis: &ql_analysis::Analysis) -> String {
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
