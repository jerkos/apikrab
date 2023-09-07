use clap::{Args, Subcommand};
use crate::commands::flow::list::FlowListArgs;

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
