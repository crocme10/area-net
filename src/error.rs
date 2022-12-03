//! Main Error Type

use error_stack::Context;
use serde::Serialize;
use std::fmt;

/// Main Error Type
#[derive(Debug, Copy, Clone, Serialize)]
pub struct Error;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("KV Error")
    }
}

// It's also possible to implement `Error` instead.
impl Context for Error {}
