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
#[path = "build_single_source_input_tests.rs"]
mod tests;
