//! Pure file invocation compilation. Descriptions never grant authority or execute code.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod calling;
mod model;
mod planner;
pub use model::*;
pub use planner::compile_invocation;
