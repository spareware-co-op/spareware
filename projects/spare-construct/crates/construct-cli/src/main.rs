//! Spare Construct: headless command line.
//!
//! Runs every Transport command without a screen: one command, a batch of commands in JSON lines, the command list with its
//! schemas, or an MCP server so an AI agent can use the same commands. Map: "Headless API, AI and add-ins".

mod cli;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(cli::run(args));
}
