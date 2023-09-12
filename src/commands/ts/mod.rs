use crate::commands::ts::add_ts::AddTestSuiteArgs;
use crate::commands::ts::create::CreateTestSuiteArgs;
use clap::{Args, Subcommand};

mod add_ts;
mod create;

#[derive(Args)]
pub struct TestSuite {
    #[command(subcommand)]
    pub ts_commands: TestSuiteCommands,
}

#[derive(Subcommand)]
pub enum TestSuiteCommands {
    /// Run an action
    New(CreateTestSuiteArgs),

    AddTestSuite(AddTestSuiteArgs),
}
