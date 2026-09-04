//! SECFORGE — security MCP server aggregating the BITECO satellites.

use std::collections::HashMap;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use serde_json::json;

use secforge::cli::{Cli, Command};
use secforge::spawn;
use secforge::state::AppState;
use secforge::tools::{tools, Satellite};

fn log(msg: &str) {
    eprintln!("[secforge] {msg}");
}

/// Resolve every satellite binary; returns (state, per-satellite detection log).
async fn detect_state(cli: &Cli, scan_authorized: bool) -> AppState {
    let overrides = [
        (Satellite::Neton, &cli.neton),
        (Satellite::Firelin, &cli.firelin),
        (Satellite::Adonword, &cli.adonword),
        (Satellite::Howcueme, &cli.howcueme),
        (Satellite::Memorypool, &cli.memorypool),
    ];
    let mut bins = HashMap::new();
    for (satellite, override_path) in overrides {
        match spawn::resolve(satellite, override_path.as_ref()).await {
            Some(path) => {
                log(&format!(
                    "satellite '{}' detected: {}",
                    satellite.bin_name(),
                    path.display()
                ));
                bins.insert(satellite, path);
            }
            None => log(&format!(
                "satellite '{}' NOT found — its tools stay hidden (install it or pass --{})",
                satellite.bin_name(),
                satellite.bin_name()
            )),
        }
    }
    AppState {
        scan_authorized,
        bins,
    }
}

async fn serve(cli: &Cli, host: String, port: u16, scan_authorized: bool) -> Result<()> {
    let state = detect_state(cli, scan_authorized).await;
    let exposed = state.exposed_tools().count();
    log(&format!(
        "{exposed}/{} tools exposed across {} satellite(s); scan tools {}",
        tools().len(),
        state.bins.len(),
        if scan_authorized {
            "UNLOCKED"
        } else {
            "locked (start with --yes-i-have-permission to unlock)"
        }
    ));

    let router = secforge::mcp::mcp_router(state);
    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    log(&format!(
        "MCP endpoint listening on http://{host}:{port}/ (also /mcp)"
    ));
    axum::serve(listener, router).await?;
    Ok(())
}

async fn print_tools(cli: &Cli, scan_authorized: bool) -> Result<()> {
    let state = detect_state(cli, scan_authorized).await;
    let tools: Vec<serde_json::Value> = state
        .exposed_tools()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
                "satellite": t.satellite.bin_name(),
                "argv": t.argv,
                "scan_gated": t.scan_gated,
            })
        })
        .collect();
    println!(
        "{}",
        json!({
            "scan_authorized": state.scan_authorized,
            "satellites": state.bins.iter().map(|(s, p)| (s.bin_name().to_string(), p.display().to_string())).collect::<HashMap<String, String>>(),
            "tools": tools,
        })
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("[secforge] error: {err:#}");
        std::process::exit(2);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let scan_allowed = cli.yes_i_have_permission
        || std::env::var("SECFORGE_I_HAVE_PERMISSION").as_deref() == Ok("yes");

    match &cli.command {
        Some(Command::Serve { host, port }) => serve(&cli, host.clone(), *port, scan_allowed).await,
        Some(Command::Tools) => print_tools(&cli, scan_allowed).await,
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
