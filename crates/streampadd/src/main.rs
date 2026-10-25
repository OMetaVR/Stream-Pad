//! `streampadd` — see docs/SPEC.md §5.

use clap::Parser;
use std::process::ExitCode;
use streampadd::cli::{Cli, Command};
use streampadd::{ctl, daemon, logging};

fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::init();
    let result = match cli.command {
        None => daemon::run(cli.run),
        Some(Command::Run(args)) => daemon::run(args),
        Some(Command::Ctl { method, params }) => ctl::ctl(&method, params.as_deref()),
        Some(Command::ListDevices { json }) => ctl::list_devices(json),
    };
    result.unwrap_or_else(|e| {
        eprintln!("streampadd: {e:#}");
        ExitCode::FAILURE
    })
}
