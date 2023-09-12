use crate::commands::flow::list::FlowListArgs;
use clap::{Args, Subcommand};

mod list;

#[derive(Args)]
pub struct Flow {
    #[command(subcommand)]
    pub flow_commands: FlowCommands,
}

#[derive(Subcommand)]
pub enum FlowCommands {
    /// Run an action
    List(FlowListArgs),
}
