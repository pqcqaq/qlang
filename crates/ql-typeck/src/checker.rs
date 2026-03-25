use ql_diagnostics::Diagnostic;
use ql_hir::Module;
use ql_resolve::ResolutionMap;

use crate::{duplicates, typing};

/// Run the current Phase 2 semantic checks over lowered HIR plus name-resolution data.
pub fn check_module(module: &Module, resolution: &ResolutionMap) -> Vec<Diagnostic> {
    let mut diagnostics = duplicates::check_module(module);
    diagnostics.extend(typing::check_module(module, resolution));
    diagnostics
}
