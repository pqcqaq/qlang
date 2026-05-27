use std::path::PathBuf;

mod emission;
mod reference_prep;

#[cfg(test)]
pub(crate) use emission::render_interface_artifact;
pub(crate) use emission::{emit_package_interface_path, emit_package_interface_path_quiet};
#[cfg(test)]
pub(crate) use reference_prep::ReferenceInterfacePrepFailure;
pub(crate) use reference_prep::{
    ReferenceInterfacePrepError, ReferenceInterfacePrepFailureKind,
    prepare_reference_interfaces_for_manifests_quiet,
};

#[derive(Debug)]
pub(crate) enum EmitPackageInterfaceResult {
    Wrote(PathBuf),
    UpToDate(PathBuf),
}

#[derive(Debug)]
pub(crate) enum EmitPackageInterfaceError {
    Code {
        code: u8,
        message: Option<String>,
    },
    SourceFailure {
        code: u8,
        failure_count: usize,
        first_failing_source: Option<PathBuf>,
    },
    ManifestNotFound {
        start: PathBuf,
    },
    ManifestFailure {
        manifest_path: PathBuf,
        message: String,
    },
    NoSourceFilesFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    SourceRootFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    OutputPathFailure {
        manifest_path: Option<PathBuf>,
        output_path: PathBuf,
        message: String,
    },
}

#[cfg(test)]
#[path = "project_interfaces_tests.rs"]
mod tests;
