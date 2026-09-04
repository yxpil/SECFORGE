//! Library root: re-exported modules so integration tests can reuse the
//! registry and state helpers without spawning the binary.

pub mod cli;
pub mod mcp;
pub mod spawn;
pub mod state;
pub mod tools;
