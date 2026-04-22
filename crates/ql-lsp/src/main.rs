use std::env;

use ql_lsp::Backend;
use tower_lsp::{LspService, Server};

const SERVER_NAME: &str = "qlsp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    let remaining = env::args().skip(1).collect::<Vec<_>>();
    if let Some(command) = remaining.first() {
        if is_version_command(command) {
            if remaining.len() > 1 {
                eprintln!(
                    "error: `qlsp {command}` does not accept additional arguments"
                );
                std::process::exit(1);
            }
            println!("{}", version_text(SERVER_NAME));
            return;
        }
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn is_version_command(command: &str) -> bool {
    matches!(command, "--version" | "-V" | "version")
}

fn version_text(binary_name: &str) -> String {
    format!("{binary_name} {SERVER_VERSION}")
}

#[cfg(test)]
mod tests {
    use super::{is_version_command, version_text};

    #[test]
    fn version_text_includes_package_version() {
        assert_eq!(
            version_text("qlsp"),
            format!("qlsp {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn version_command_recognizes_global_aliases() {
        for command in ["--version", "-V", "version"] {
            assert!(is_version_command(command), "expected {command} to be recognized");
        }
        assert!(!is_version_command("stdio"));
    }
}
