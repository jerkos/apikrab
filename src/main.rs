mod commands;
mod complete;
mod db;
mod http;
mod json_path;
mod ui;
mod utils;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use crate::commands::history::{History, HistoryCommands};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use lazy_static::lazy_static;

use crate::commands::flow::{Flow, FlowCommands};
use crate::commands::project::{Project, ProjectCommands};
use crate::commands::run::{Run, RunCommands};
use crate::commands::ts::{TestSuite, TestSuiteCommands};
use crate::complete::{complete_update, load_complete_entities};
use crate::db::db_handler::DBHandler;
use crate::ui::run_ui::UIRunner;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create or update a new project with specified parameters
    Project(Project),

    /// Run a project action, flow or test suite
    Run(Run),

    /// Get information about existing flows
    Flow(Flow),

    /// Test suite information
    TestSuite(TestSuite),

    /// List all history call
    History(History),

    /// Reload completion script (only for oh-my-zsh)
    Complete { shell: Shell },

    /// Print the completion script in stdout
    PrintCompleteScript { shell: Shell },
}

lazy_static! {
    pub static ref HOME_DIR: PathBuf = std::env::home_dir().unwrap();
    pub static ref ALL_ENTITES: HashMap<&'static str, Vec<String>> = load_complete_entities();
    pub static ref ACTIONS: Vec<&'static str> = ALL_ENTITES
        .get("actions")
        .unwrap()
        .iter()
        .map(|s| s.as_str())
        .collect();
    pub static ref PROJECTS: Vec<&'static str> = ALL_ENTITES
        .get("projects")
        .unwrap()
        .iter()
        .map(|s| s.as_str())
        .collect();
    pub static ref FLOWS: Vec<&'static str> = ALL_ENTITES
        .get("flows")
        .unwrap()
        .iter()
        .map(|s| s.as_str())
        .collect();
    pub static ref TEST_SUITE: Vec<&'static str> = ALL_ENTITES
        .get("test_suite")
        .unwrap()
        .iter()
        .map(|s| s.as_str())
        .collect();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // init database if needed
    let mut db_handler = DBHandler::new();
    db_handler.init_db().await?;

    // init http requester
    let requester = http::Api::new(&db_handler);

    // parse cli args
    let mut cli: Cli = Cli::parse();

    match &mut cli.commands {
        Commands::Project(project) => match &mut project.project_commands {
            ProjectCommands::New(create_project_args) => {
                create_project_args.create(&db_handler).await?;
            }
            ProjectCommands::RmAction(rm_action_args) => {
                rm_action_args.rm_action(&db_handler).await?;
            }
            ProjectCommands::AddAction(add_action_args) => {
                add_action_args.add_action(&db_handler).await?;
            }
            ProjectCommands::List(list_projects) => {
                list_projects.list_projects(&db_handler).await?;
            }
            ProjectCommands::Info(project_info_args) => {
                project_info_args.show_info(&db_handler).await?;
            }
            ProjectCommands::Ui => {
                let projects = db_handler.get_projects().await?;
                let mut ui = commands::project::project_ui::ProjectUI::new(projects, db_handler);
                ui.run_ui()?;
            }
        },
        Commands::Run(run) => match &mut run.run_commands {
            RunCommands::Action(run_action_args) => {
                run_action_args.run_action(&requester).await?;
            }
            RunCommands::Flow(run_flow_args) => {
                run_flow_args.run_flow(&db_handler, &requester).await?;
            }
            RunCommands::TestSuite(test_suite_args) => {
                test_suite_args.run_test_suite(&requester).await?;
            }
        },
        Commands::Flow(flow) => match &mut flow.flow_commands {
            FlowCommands::List(flow_list_args) => {
                flow_list_args.list_flows(&db_handler).await?;
            }
        },
        Commands::TestSuite(test_suite) => match &mut test_suite.ts_commands {
            TestSuiteCommands::New(create_test_suite_args) => {
                create_test_suite_args
                    .upsert_test_suite(&db_handler)
                    .await?;
            }
            TestSuiteCommands::AddTestSuite(add_test_suite_args) => {
                add_test_suite_args.add_test_suite(&db_handler).await?;
            }
        },
        Commands::History(history) => match &mut history.history_commands {
            HistoryCommands::Ui => {
                let histories = db_handler.get_history(None).await?;
                let mut ui = commands::history::list_ui::HistoryUI::new(histories);
                ui.run_ui()?;
            }
            HistoryCommands::List(list_args) => {
                list_args.list_history(&db_handler).await?;
            }
        },
        &mut Commands::PrintCompleteScript { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "apicrab".to_string(),
                &mut io::stdout(),
            );
        }
        &mut Commands::Complete { shell } => {
            // write action
            match shell {
                Shell::Bash => {}
                Shell::Elvish => {}
                Shell::Fish => {}
                Shell::PowerShell => {}
                Shell::Zsh => {
                    complete_update(&db_handler.conn.unwrap()).await?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
