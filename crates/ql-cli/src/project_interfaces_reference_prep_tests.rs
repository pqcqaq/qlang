use super::*;

#[test]
fn quiet_reference_prep_accepts_empty_manifest_list() {
    prepare_reference_interfaces_for_manifests_quiet(&[])
        .expect("empty quiet reference prep should succeed");
}
