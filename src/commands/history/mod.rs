pub(crate) mod list;

use clap::{Args, Subcommand};

use crate::commands::history::list::HistoryArgs;

#[derive(Args)]
pub struct History {
    #[command(subcommand)]
    pub history_commands: HistoryCommands,
}

#[derive(Subcommand)]
pub enum HistoryCommands {
    /// Show history
    List(HistoryArgs),
}
