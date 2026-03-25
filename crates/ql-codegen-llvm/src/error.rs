use ql_diagnostics::Diagnostic;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodegenError {
    diagnostics: Vec<Diagnostic>,
}

impl CodegenError {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
