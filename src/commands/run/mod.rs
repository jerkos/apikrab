pub(crate) mod _http_result;
pub(crate) mod _printer;
pub(crate) mod _progress_bar;
pub(crate) mod _run_helper;
pub(crate) mod _test_checker;
pub(crate) mod action;
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
    /// get
    #[command(alias = "GET")]
    Get(Box<RunActionArgs>),
    /// Run an action
    Action(Box<RunActionArgs>),
    /// Run a saved test suite
    TestSuite(test_suite::TestSuiteArgs),
}
