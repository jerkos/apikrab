pub(crate) mod action;
mod flow;

use clap::{Args, Subcommand};

use crate::commands::run::action::RunActionArgs;

#[derive(Args)]
pub struct Run {
    #[command(subcommand)]
    pub run_commands: RunCommands,
}

#[derive(Subcommand)]
pub enum RunCommands {
    /// Run an action
    Action(RunActionArgs),

    Flow(flow::RunFlowArgs),
}
