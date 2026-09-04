//! Command-line interface definition (English help per ecosystem conventions).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "secforge",
    version,
    about = "Security MCP server for AI agents — aggregates the BITECO satellites as MCP tools (JSON-RPC over Streamable HTTP)",
    after_help = "The MCP endpoint answers on http://<host>:<port>/ and /mcp (Streamable HTTP).\n\
BIT: add the server URL in the MCP servers page, or let discovery find it on 127.0.0.1:8756.\n\n\
Scan tools (neton_netscan / neton_portscan / neton_device / firelin_portscan /\n\
firelin_subdns / firelin_dirscan) require explicit authorization:\n\
pass --yes-i-have-permission or set SECFORGE_I_HAVE_PERMISSION=yes — only scan\n\
networks you own or are authorized to test."
)]
pub struct Cli {
    /// Acknowledge you are authorized to run network scans from this server.
    #[arg(long, global = true)]
    pub yes_i_have_permission: bool,
    /// Explicit path to the neton binary (default: PATH lookup).
    #[arg(long, global = true, value_name = "PATH")]
    pub neton: Option<PathBuf>,
    /// Explicit path to the firelin binary (default: PATH lookup).
    #[arg(long, global = true, value_name = "PATH")]
    pub firelin: Option<PathBuf>,
    /// Explicit path to the adonword binary (default: PATH lookup).
    #[arg(long, global = true, value_name = "PATH")]
    pub adonword: Option<PathBuf>,
    /// Explicit path to the howcueme binary (default: PATH lookup).
    #[arg(long, global = true, value_name = "PATH")]
    pub howcueme: Option<PathBuf>,
    /// Explicit path to the memorypool binary (default: PATH lookup).
    #[arg(long, global = true, value_name = "PATH")]
    pub memorypool: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the MCP server (Streamable HTTP JSON-RPC on / and /mcp)
    Serve {
        /// Bind address (default 127.0.0.1 — loopback only, no token support)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Bind port
        #[arg(long, default_value_t = 8756)]
        port: u16,
    },
    /// Print the tools that would be exposed, as JSON
    Tools,
}
