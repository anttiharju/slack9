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
    /// Quit the application
    Quit,
}

pub fn parse_args() {
    Cli::parse();
}

/// Returns the names of all TUI subcommands (e.g. `["poll", "past", "search", ...]`).
pub fn tui_command_names() -> Vec<String> {
    Cli::command().get_subcommands().map(|cmd| cmd.get_name().to_string()).collect()
}
