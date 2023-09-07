pub(crate) mod list;
pub(crate) mod list_ui;

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
    /// Run history ui
    Ui

}
