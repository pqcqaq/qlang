use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct DependencyExternOwner {
    pub(crate) package_name: String,
    pub(crate) manifest_path: PathBuf,
}

pub(crate) fn record_dependency_extern_declaration(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    symbol_name: &str,
    declaration: String,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    if let Some(owner) = owners_by_symbol.get(symbol_name) {
        return Err((symbol_name.to_owned(), owner.clone()));
    }
    owners_by_symbol.insert(
        symbol_name.to_owned(),
        DependencyExternOwner {
            package_name: dependency_package.to_owned(),
            manifest_path: dependency_manifest_path.to_path_buf(),
        },
    );
    declarations.push(declaration.trim().to_owned());
    Ok(())
}
