mod _http_result;
pub(crate) mod _printer;
mod _test_checker;
pub(crate) mod action;
mod flow;
mod test_suite;

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
    /// Run a saved flow
    Flow(flow::RunFlowArgs),
    /// Run a saved test suite
    TestSuite(test_suite::TestSuiteArgs),
}
