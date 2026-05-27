use super::*;
use ql_driver::BuildCHeaderOptions;

#[test]
fn build_rerun_hint_preserves_build_options() {
    let options = BuildOptions {
        emit: BuildEmit::DynamicLibrary,
        profile: BuildProfile::Release,
        output: Some(PathBuf::from("dist/libdemo.so")),
        c_header: Some(BuildCHeaderOptions {
            output: Some(PathBuf::from("include/demo.h")),
            surface: CHeaderSurface::Both,
        }),
        ..BuildOptions::default()
    };

    let hint = build_rerun_hint(
        Path::new("src/lib.ql"),
        &options,
        true,
        "after fixing the package sources",
    );

    assert_eq!(
        hint,
        "hint: rerun `ql build src/lib.ql --emit dylib --release --output dist/libdemo.so --header-surface both --header-output include/demo.h --emit-interface` after fixing the package sources"
    );
}

#[test]
fn build_failure_notes_use_shared_text_shapes() {
    assert_eq!(
        build_path_note("failing package manifest", Path::new("pkg/qlang.toml")),
        "note: failing package manifest: pkg/qlang.toml"
    );
    assert_eq!(
        build_artifact_remaining_note(Path::new("target/ql/debug/app.ll")),
        "note: build artifact remains at `target/ql/debug/app.ll`"
    );
}
