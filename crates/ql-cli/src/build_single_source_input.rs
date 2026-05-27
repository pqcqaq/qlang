use std::borrow::Cow;
use std::fs;
use std::path::Path;

use ql_driver::BuildError;

use crate::build_source_rewrites::local_generic_source_override;

#[derive(Debug)]
pub(crate) struct PreparedSingleSource<'a> {
    source: Cow<'a, str>,
}

impl<'a> PreparedSingleSource<'a> {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
}

pub(crate) fn prepare_single_source<'a>(
    path: &Path,
    source_override: Option<&'a str>,
) -> Result<PreparedSingleSource<'a>, BuildError> {
    let Some(source_override) = source_override else {
        return prepare_single_source_from_file(path);
    };
    Ok(PreparedSingleSource {
        source: Cow::Borrowed(source_override),
    })
}

fn prepare_single_source_from_file(
    path: &Path,
) -> Result<PreparedSingleSource<'static>, BuildError> {
    if !path.is_file() {
        return Err(BuildError::InvalidInput(format!(
            "`{}` is not a file",
            path.display()
        )));
    }

    let source = fs::read_to_string(path).map_err(|error| BuildError::Io {
        path: path.to_path_buf(),
        error,
    })?;
    let source = local_generic_source_override(&source).unwrap_or(source);
    Ok(PreparedSingleSource {
        source: Cow::Owned(source),
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ql_driver::BuildError;

    use super::prepare_single_source;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("ql-cli-{name}-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("create test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(&path, contents).expect("write test source");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn source_override_bypasses_file_lookup() {
        let source = "fn main() -> Int { return 7 }\n";
        let missing_path = std::env::temp_dir().join("ql-cli-missing-source-override.ql");

        let prepared = prepare_single_source(&missing_path, Some(source))
            .expect("source override should not read from disk");

        assert!(matches!(prepared.source, Cow::Borrowed(_)));
        assert_eq!(prepared.source(), source);
    }

    #[test]
    fn missing_file_reports_invalid_input() {
        let temp = TestDir::new("single-source-missing-file");
        let path = temp.path().join("missing.ql");

        let error = prepare_single_source(&path, None)
            .expect_err("missing file should report invalid input");

        match error {
            BuildError::InvalidInput(message) => {
                assert!(message.contains("is not a file"));
                assert!(message.contains("missing.ql"));
            }
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    #[test]
    fn plain_file_source_is_preserved() {
        let temp = TestDir::new("single-source-plain-file");
        let source = "fn main() -> Int { return 1 }\n";
        let path = temp.write("main.ql", source);

        let prepared =
            prepare_single_source(&path, None).expect("plain source should load from disk");

        assert_eq!(prepared.source(), source);
    }

    #[test]
    fn file_source_applies_local_generic_rewrite() {
        let temp = TestDir::new("single-source-local-generic");
        let path = temp.write(
            "main.ql",
            r#"
fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

fn len[T, N](values: [T; N]) -> Int {
    return N
}

fn main() -> Int {
    return first([10, 20, 30]) + len([1, 2, 3, 4])
}
"#,
        );

        let prepared =
            prepare_single_source(&path, None).expect("generic source should load from disk");

        assert!(
            prepared
                .source()
                .contains("fn __ql_bridge_local_first__generic_Int_3"),
            "generic source should include a specialized first forwarder:\n{}",
            prepared.source()
        );
        assert!(
            prepared
                .source()
                .contains("fn __ql_bridge_local_len__generic_Int_4"),
            "generic source should include a specialized len forwarder:\n{}",
            prepared.source()
        );
        assert!(
            prepared
                .source()
                .contains("__ql_bridge_local_first__generic_Int_3([10, 20, 30])"),
            "generic source should rewrite the first call:\n{}",
            prepared.source()
        );
    }
}
