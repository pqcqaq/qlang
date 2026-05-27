use super::*;

#[test]
fn reference_interface_sync_note_mentions_owner_and_reference() {
    assert_eq!(
        format_reference_interface_sync_note(Path::new("app/qlang.toml"), "std/core"),
        "note: while syncing referenced package `std/core` from `app/qlang.toml`"
    );
}
