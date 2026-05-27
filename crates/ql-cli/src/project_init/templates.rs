use std::fs;
use std::path::Path;

use crate::cli_utils::normalize_path;

pub(super) struct PackageSources {
    pub(super) package_source: String,
    pub(super) main_source: String,
    pub(super) test_source: String,
}

pub(crate) fn default_package_main_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_source() -> &'static str {
    "pub fn run() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_test_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_sources() -> PackageSources {
    PackageSources {
        package_source: default_package_source().to_owned(),
        main_source: default_package_main_source().to_owned(),
        test_source: default_package_test_source().to_owned(),
    }
}

pub(super) fn stdlib_package_sources(stdlib_root: &Path) -> Result<PackageSources, String> {
    let starter_root = stdlib_root.join("examples").join("starter");
    Ok(PackageSources {
        package_source: read_starter_source(&starter_root.join("src").join("lib.ql"))?,
        main_source: read_starter_source(&starter_root.join("src").join("main.ql"))?,
        test_source: read_starter_source(&starter_root.join("tests").join("smoke.ql"))?,
    })
}

fn read_starter_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "stdlib starter template `{}` is not available: {error}",
            normalize_path(path)
        )
    })
}

#[cfg(test)]
#[path = "templates_tests.rs"]
mod tests;
