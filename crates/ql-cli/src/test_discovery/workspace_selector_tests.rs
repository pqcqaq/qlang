use crate::project_workspace::WorkspaceMemberLookupError;

use super::workspace_selector::workspace_selector_lookup_error_json_fields;

#[test]
fn workspace_selector_lookup_error_fields_report_missing_as_empty_selector() {
    assert_eq!(
        workspace_selector_lookup_error_json_fields(&WorkspaceMemberLookupError::Missing),
        ("selector", Some(0))
    );
}

#[test]
fn workspace_selector_lookup_error_fields_report_ambiguous_match_count() {
    let error = WorkspaceMemberLookupError::Ambiguous {
        matches: vec!["a/qlang.toml".to_owned(), "b/qlang.toml".to_owned()],
    };

    assert_eq!(
        workspace_selector_lookup_error_json_fields(&error),
        ("selector", Some(2))
    );
}

#[test]
fn workspace_selector_lookup_error_fields_report_inspection_as_manifest_failure() {
    let error = WorkspaceMemberLookupError::InspectionFailure {
        member: "broken/qlang.toml".to_owned(),
        message: "boom".to_owned(),
    };

    assert_eq!(
        workspace_selector_lookup_error_json_fields(&error),
        ("manifest", None)
    );
}
