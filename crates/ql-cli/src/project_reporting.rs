use std::path::Path;

use ql_project::InterfaceArtifactStaleReason;

use crate::cli_utils::normalize_path;

pub(crate) fn report_workspace_member_failure(manifest_path: &Path, hint_line: Option<&str>) {
    eprintln!(
        "note: failing workspace member manifest: {}",
        normalize_path(manifest_path)
    );
    if let Some(hint_line) = hint_line {
        eprintln!("{hint_line}");
    }
}

pub(crate) fn report_interface_artifact_failure(
    error_line: &str,
    detail: Option<&str>,
    stale_reasons: &[InterfaceArtifactStaleReason],
    notes: &[&str],
    hint_line: &str,
) {
    eprint!(
        "{}",
        render_interface_artifact_failure(error_line, detail, stale_reasons, notes, hint_line)
    );
}

fn render_interface_artifact_failure(
    error_line: &str,
    detail: Option<&str>,
    stale_reasons: &[InterfaceArtifactStaleReason],
    notes: &[&str],
    hint_line: &str,
) -> String {
    let mut rendered = String::new();
    rendered.push_str(error_line);
    rendered.push('\n');
    if let Some(detail) = detail {
        rendered.push_str("detail: ");
        rendered.push_str(detail);
        rendered.push('\n');
    }
    for reason in stale_reasons {
        match reason {
            InterfaceArtifactStaleReason::ManifestNewer { path } => {
                rendered.push_str("reason: manifest newer than artifact: ");
                rendered.push_str(&normalize_path(path));
                rendered.push('\n');
            }
            InterfaceArtifactStaleReason::SourceNewer { path } => {
                rendered.push_str("reason: source newer than artifact: ");
                rendered.push_str(&normalize_path(path));
                rendered.push('\n');
            }
        }
    }
    for note in notes {
        rendered.push_str(note);
        rendered.push('\n');
    }
    rendered.push_str(hint_line);
    rendered.push('\n');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn interface_artifact_failure_includes_detail_reasons_notes_and_hint() {
        let rendered = render_interface_artifact_failure(
            "error: stale interface",
            Some("checksum mismatch"),
            &[
                InterfaceArtifactStaleReason::ManifestNewer {
                    path: PathBuf::from("pkg/qlang.toml"),
                },
                InterfaceArtifactStaleReason::SourceNewer {
                    path: PathBuf::from("pkg/src/lib.ql"),
                },
            ],
            &["note: failing package manifest: pkg/qlang.toml"],
            "hint: rerun `ql project emit-interface pkg/qlang.toml`",
        );

        assert_eq!(
            rendered,
            "error: stale interface\n\
detail: checksum mismatch\n\
reason: manifest newer than artifact: pkg/qlang.toml\n\
reason: source newer than artifact: pkg/src/lib.ql\n\
note: failing package manifest: pkg/qlang.toml\n\
hint: rerun `ql project emit-interface pkg/qlang.toml`\n"
        );
    }
}
