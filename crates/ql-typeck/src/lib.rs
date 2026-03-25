mod checker;
mod duplicates;
mod types;
mod typing;

pub use checker::{TypeckResult, analyze_module, check_module};
pub use types::Ty;
