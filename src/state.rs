//! Shared server state: scan authorization + detected satellite binaries.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::tools::{tools, Satellite, ToolDef};

/// Immutable state shared by all MCP handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Mirrors `--yes-i-have-permission` / `SECFORGE_I_HAVE_PERMISSION=yes`.
    pub scan_authorized: bool,
    /// Resolved binary path per detected satellite (probe passed at startup).
    pub bins: HashMap<Satellite, PathBuf>,
}

impl AppState {
    /// Tools whose satellite binary was detected; stable registry order.
    pub fn exposed_tools(&self) -> impl Iterator<Item = &ToolDef> {
        tools()
            .iter()
            .filter(|t| self.bins.contains_key(&t.satellite))
    }

    /// Look up one tool by MCP name among the exposed ones.
    pub fn find_tool(&self, name: &str) -> Option<&ToolDef> {
        self.exposed_tools().find(|t| t.name == name)
    }
}
