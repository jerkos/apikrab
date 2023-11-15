mod commands;
mod db;
pub mod domain;
mod http;
mod json_path;
mod ui;
mod utils;
use std::io;
use std::path::PathBuf;

use crate::commands::history::{History, HistoryCommands};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use commands::run::action::RunActionArgs;
use futures::StreamExt;
use http::Verb;
use lazy_static::lazy_static;
use sqlx::Either::{Left, Right};
use sqlx::{Column, Executor, Row};

use crate::commands::project::{Project, ProjectCommands};
use crate::commands::run::{Run, RunCommands};
use crate::commands::ts::{TestSuite, TestSuiteCommands};
use crate::db::db_handler::DBHandler;
use crate::db::dto::Project as DtoProject;
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
    #[command(alias = "r")]
    Run(Run),
    /// Test suite information
    #[command(alias = "ts")]
    TestSuite(TestSuite),
    /// List all history call
    #[command(alias = "h")]
    History(History),
    /// Print the completion script in stdout
    PrintCompleteScript { shell: Shell },
    /// Exec sql command (for debug purpose)
    Sql { q: String },
}

lazy_static! {
    pub static ref HOME_DIR: PathBuf = home::home_dir().unwrap();
    pub static ref DEFAULT_PROJECT: DtoProject = DtoProject {
        id: None,
        name: "DEFAULT".to_string(),
        main_url: "".to_string(),
        conf: None,
        created_at: None,
        updated_at: None
    };
}

async fn run_wrapper(
    run_action_args: &mut Box<RunActionArgs>,
    v: Option<Verb>,
    db_handler: &DBHandler,
) {
    let requester = http::Api::new(run_action_args.timeout, run_action_args.insecure);
    run_action_args.verb = v.map(|v| v.to_string());
    let _ = run_action_args
        .run_action(&requester, db_handler, None, None)
        .await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // init database if needed
    let mut db_handler = DBHandler::default();
    db_handler.init_db().await?;

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
                let mut projects = db_handler.get_projects().await?;
                projects.push(DEFAULT_PROJECT.clone());
                let mut ui = commands::project::project_ui::ProjectUI::new(projects, db_handler);
                ui.run_ui()?;
            }
        },
        Commands::Run(run) => match &mut run.run_commands {
            // init http requester
            RunCommands::Action(run_action_args) => {
                run_wrapper(run_action_args, None, &db_handler).await;
            }
            RunCommands::TestSuite(test_suite_args) => {
                let requester = http::Api::new(Some(10), true);
                test_suite_args
                    .run_test_suite(&requester, &db_handler)
                    .await?;
            }
            RunCommands::Get(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Get), &db_handler).await;
            }
            RunCommands::Post(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Post), &db_handler).await;
            }
            RunCommands::Put(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Put), &db_handler).await;
            }
            RunCommands::Delete(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Delete), &db_handler).await;
            }
        },
        Commands::TestSuite(test_suite) => match &mut test_suite.ts_commands {
            TestSuiteCommands::New(create_test_suite_args) => {
                create_test_suite_args
                    .upsert_test_suite(&db_handler)
                    .await?;
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
                "apikrab".to_string(),
                &mut io::stdout(),
            );
        }
        Commands::Sql { q } => {
            let r = db_handler.conn.unwrap().fetch_many(q.as_str());
            r.for_each(|row| async {
                if let Ok(r) = row {
                    match r {
                        Left(sqlite_results) => {
                            println!(
                                "{} changes, {} last insert id",
                                sqlite_results.rows_affected(),
                                sqlite_results.last_insert_rowid()
                            );
                        }
                        Right(sqlite_row) => {
                            println!(
                                "{}",
                                sqlite_row
                                    .columns()
                                    .iter()
                                    .fold("".to_string(), |acc, col| {
                                        format!(
                                            "{}{}: {}, ",
                                            acc,
                                            col.name(),
                                            sqlite_row
                                                .try_get::<String, _>(col.name())
                                                .unwrap_or_else(|_| sqlite_row
                                                    .try_get::<i32, _>(col.name())
                                                    .map(|i| i.to_string())
                                                    .unwrap())
                                        )
                                    })
                            );
                        }
                    }
                }
            })
            .await;
        }
    }

    Ok(())
}
