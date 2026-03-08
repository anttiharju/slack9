use clap::{CommandFactory, Parser, Subcommand};

mod style;
use style::get_style;

#[derive(Parser)]
#[command(styles = get_style())]
struct Cli {
    #[command(subcommand)]
    command: Option<TuiCommand>,
}

#[derive(Subcommand)]
pub enum TuiCommand {
    /// Set the poll interval
    Poll { value: Option<String> },
    /// Set the past duration
    Time { value: Option<String> },
    /// Search for messages by user
    Search { query: Vec<String> },
    /// Switch to a channel
    Channel { name: Option<String> },
    /// Define a reaction filter
    Reaction { name: String, emoji: String },
    /// Quit the application
    Quit,
}

pub fn parse_args() {
    Cli::parse();
}

/// Returns TUI subcommand names split into (unique_prefix, rest) pairs.
/// The unique prefix is the shortest prefix that distinguishes each command from all others.
pub fn tui_command_prefixes() -> Vec<(String, String)> {
    let names: Vec<String> = Cli::command().get_subcommands().map(|cmd| cmd.get_name().to_string()).collect();
    names
        .iter()
        .map(|name| {
            let mut len = 1;
            while len < name.len() {
                let prefix = &name[..len];
                if names.iter().filter(|other| *other != name && other.starts_with(prefix)).count() == 0 {
                    break;
                }
                len += 1;
            }
            (name[..len].to_string(), name[len..].to_string())
        })
        .collect()
}
