mod common;

use common::request::{
    TempDir, initialized_service_with_open_documents, nth_offset, offset_to_position,
    prepare_type_hierarchy_via_request, subtypes_via_request, supertypes_via_request,
};
use tower_lsp::lsp_types::SymbolKind;

#[tokio::test]
async fn type_hierarchy_request_reports_same_file_trait_and_type_edges() {
    let temp = TempDir::new("ql-lsp-type-hierarchy-trait-type");
    let source = r#"
trait Printable {
    fn print(self) -> Int
}

struct User {
    id: Int,
}

enum Status {
    Ready,
}

impl Printable for User {
    fn print(self) -> Int {
        return self.id
    }
}

impl Printable for Status {
    fn print(self) -> Int {
        return 0
    }
}
"#;
    let path = temp.write("types.ql", source);
    let uri = tower_lsp::lsp_types::Url::from_file_path(&path).expect("uri should be valid");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let trait_position = offset_to_position(source, nth_offset(source, "Printable", 1));
    let trait_items = prepare_type_hierarchy_via_request(&mut service, uri.clone(), trait_position)
        .await
        .expect("prepareTypeHierarchy should return trait");
    assert_eq!(trait_items.len(), 1);
    assert_eq!(trait_items[0].name, "Printable");
    assert_eq!(trait_items[0].kind, SymbolKind::INTERFACE);

    let subtypes = subtypes_via_request(&mut service, trait_items[0].clone())
        .await
        .expect("subtypes should return implementing types");
    assert_eq!(subtypes.len(), 2);
    assert_eq!(subtypes[0].name, "User");
    assert_eq!(subtypes[0].kind, SymbolKind::STRUCT);
    assert_eq!(subtypes[1].name, "Status");
    assert_eq!(subtypes[1].kind, SymbolKind::ENUM);

    let user_position = offset_to_position(source, nth_offset(source, "User", 1));
    let user_items = prepare_type_hierarchy_via_request(&mut service, uri, user_position)
        .await
        .expect("prepareTypeHierarchy should return struct");
    let supertypes = supertypes_via_request(&mut service, user_items[0].clone())
        .await
        .expect("supertypes should return implemented traits");
    assert_eq!(supertypes.len(), 1);
    assert_eq!(supertypes[0].name, "Printable");
    assert_eq!(supertypes[0].kind, SymbolKind::INTERFACE);
}

#[tokio::test]
async fn type_hierarchy_request_reports_same_file_alias_edges() {
    let temp = TempDir::new("ql-lsp-type-hierarchy-alias");
    let source = r#"
struct User {
    id: Int,
}

type Alias = User
"#;
    let path = temp.write("alias.ql", source);
    let uri = tower_lsp::lsp_types::Url::from_file_path(&path).expect("uri should be valid");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let user_position = offset_to_position(source, nth_offset(source, "User", 1));
    let user_items = prepare_type_hierarchy_via_request(&mut service, uri.clone(), user_position)
        .await
        .expect("prepareTypeHierarchy should return struct");
    let subtypes = subtypes_via_request(&mut service, user_items[0].clone())
        .await
        .expect("subtypes should return aliases");
    assert_eq!(subtypes.len(), 1);
    assert_eq!(subtypes[0].name, "Alias");
    assert_eq!(subtypes[0].kind, SymbolKind::CLASS);

    let alias_position = offset_to_position(source, nth_offset(source, "Alias", 1));
    let alias_items = prepare_type_hierarchy_via_request(&mut service, uri, alias_position)
        .await
        .expect("prepareTypeHierarchy should return alias");
    let supertypes = supertypes_via_request(&mut service, alias_items[0].clone())
        .await
        .expect("supertypes should return alias target");
    assert_eq!(supertypes.len(), 1);
    assert_eq!(supertypes[0].name, "User");
    assert_eq!(supertypes[0].kind, SymbolKind::STRUCT);
}
