mod error;
mod parser;

pub use error::ParseError;
pub use parser::{parse_interface_source, parse_source};
