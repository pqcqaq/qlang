use crate::cli_version::CLI_VERSION;

pub(crate) fn print_usage() {
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
    eprintln!("  ql project dependents [file-or-dir] [--name <package>|--package <name>] [--json]");
    eprintln!(
        "  ql project dependencies [file-or-dir] [--name <package>|--package <name>] [--json]"
    );
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
