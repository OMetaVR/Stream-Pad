//! Command line (docs/SPEC.md §5).

use clap::{Args, Parser, Subcommand};
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "streampadd", version, about = "StreamPad daemon: turns your numpad into a profile-based macropad")]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub run: RunArgs,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the daemon (the default when no subcommand is given).
    Run(RunArgs),
    /// Send one request to the running daemon and print the JSON result. `ctl watch` prints events.
    Ctl {
        /// IPC method (e.g. getState, setProfile, simulateKey) or `watch`.
        method: String,
        /// JSON params, e.g. '{"delta":1}'. For `watch`: extra hello params such as '{"trace":true}'.
        params: Option<String>,
    },
    /// List input devices and whether StreamPad would grab them (grabs nothing).
    ListDevices {
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Default, Args)]
pub struct RunArgs {
    /// Don't spawn the on-screen overlay.
    #[arg(long)]
    pub no_overlay: bool,
    /// Don't touch input devices (no grabs, no virtual keyboard); pad keys can still be simulated over IPC.
    #[arg(long)]
    pub no_input: bool,
    /// Only consider the device with this id, /dev/input/eventN path or name (testing).
    #[arg(long, value_name = "ID|PATH|NAME")]
    pub only_device: Option<String>,
    /// Development safety valve: release all grabs and exit after this many seconds.
    #[arg(long, value_name = "SECS", value_parser = parse_secs)]
    pub grab_timeout: Option<Duration>,
    /// Stay in the foreground. The daemon never forks; accepted for compatibility with service managers.
    #[arg(long)]
    pub foreground: bool,
}

fn parse_secs(s: &str) -> Result<Duration, String> {
    let secs: f64 = s.parse().map_err(|_| format!("not a number of seconds: {s}"))?;
    if secs.is_finite() && secs > 0.0 {
        Ok(Duration::from_secs_f64(secs))
    } else {
        Err("must be a positive number of seconds".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_is_default_and_flags_parse() {
        let cli = Cli::try_parse_from(["streampadd", "--no-input", "--grab-timeout", "2.5"]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.run.no_input);
        assert_eq!(cli.run.grab_timeout, Some(Duration::from_millis(2500)));

        let cli = Cli::try_parse_from(["streampadd", "run", "--only-device", "SPTEST pad"]).unwrap();
        let Some(Command::Run(args)) = cli.command else { panic!("expected run") };
        assert_eq!(args.only_device.as_deref(), Some("SPTEST pad"));
    }

    #[test]
    fn ctl_takes_method_and_optional_json() {
        let cli = Cli::try_parse_from(["streampadd", "ctl", "setProfile", r#"{"delta":1}"#]).unwrap();
        let Some(Command::Ctl { method, params }) = cli.command else { panic!("expected ctl") };
        assert_eq!(method, "setProfile");
        assert_eq!(params.as_deref(), Some(r#"{"delta":1}"#));
        assert!(Cli::try_parse_from(["streampadd", "--grab-timeout", "0"]).is_err());
    }
}
