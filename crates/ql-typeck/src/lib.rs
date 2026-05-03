mod checker;
mod duplicates;
mod types;
mod typing;

pub use checker::{
    FieldTarget, MemberTarget, MethodTarget, TypeckResult, analyze_module, check_module,
};
pub use types::{Ty, TyArrayLen, lower_type};
