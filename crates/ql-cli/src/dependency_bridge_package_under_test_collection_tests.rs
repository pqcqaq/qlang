use std::path::PathBuf;

use ql_parser::parse_source;

use super::*;

#[test]
fn package_under_test_collection_includes_function_type_dependencies() {
    let package_source =
        "pub struct Box { value: Int }\npub fn open(box: Box) -> Int { return box.value }\n";
    let root_source_module = parse_source(
        "use app.open as open\n\nfn main() -> Int { return open(Box { value: 1 }) }\n",
    )
    .unwrap();
    let bridge_modules = [PackageBridgeModule {
        source: package_source.to_owned(),
        module: parse_source(package_source).unwrap(),
    }];

    let collected = match collect_package_under_test_bridge_items(
        "app",
        &PathBuf::from("app/qlang.toml"),
        &root_source_module,
        &bridge_modules,
        &[],
    ) {
        Ok(collected) => collected,
        Err(_) => panic!("package-under-test bridge collection should succeed"),
    };

    assert_eq!(collected.source_rewrites.len(), 0);
    assert_eq!(collected.function_forwarders.len(), 1);
    assert_eq!(collected.type_declarations.len(), 1);
    assert!(collected.function_forwarders[0].contains("const open: (Box) -> Int"));
    assert!(collected.type_declarations[0].contains("pub struct Box"));
}
