use std::path::PathBuf;

use super::*;

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
