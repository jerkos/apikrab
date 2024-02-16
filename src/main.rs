mod commands;
mod db;
pub mod domain;
mod http;
mod json_path;
pub mod python;
mod ui;
mod utils;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::commands::history::History;
use apikrab::serializer::{Json, Toml};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use commands::history::HistoryCommands;
use commands::run::action::RunActionArgs;
use db::db_json::FileDb;
use db::db_sqlite::SqliteDb;
use db::db_trait::{Db, FileTypeSerializer};
use futures::StreamExt;
use http::Verb;
use lazy_static::lazy_static;
use sqlx::Either::{Left, Right};
use sqlx::{Column, Executor, Row};

use crate::commands::project::{Project, ProjectCommands};
use crate::commands::run::{Run, RunCommands};
use crate::commands::ts::{TestSuite, TestSuiteCommands};
use crate::ui::run_ui::UIRunner;

/// File system serializer
#[derive(Debug, Default, Clone)]
pub enum FsSerializer {
    Json,
    #[default]
    Toml,
}

/// Database engine
#[derive(Debug, Clone)]
pub enum DBEngine {
    Sqlite,
    Fs(FsSerializer),
}

impl Default for DBEngine {
    fn default() -> Self {
        DBEngine::Fs(FsSerializer::Toml)
    }
}

/// Configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) db_engine: DBEngine,
    pub(crate) db_path: Option<String>,
}

lazy_static! {
    pub static ref HOME_DIR: PathBuf = home::home_dir().unwrap();
    pub static ref DEFAULT_DB_PATH: String = format!("{}/.config/qapi", HOME_DIR.to_str().unwrap());
    pub static ref DEFAULT_CONFIG: Mutex<Config> = Mutex::new(Config {
        db_engine: DBEngine::Fs(FsSerializer::Toml),
        db_path: Some(DEFAULT_DB_PATH.clone())
    });
}

#[allow(clippy::await_holding_lock)]
async fn get_db() -> Box<dyn Db> {
    let config = DEFAULT_CONFIG.lock().unwrap();
    match &config.db_engine {
        DBEngine::Fs(ser) => Box::new(FileDb {
            root: config.db_path.clone(),
            serializer: match ser {
                FsSerializer::Toml => FileTypeSerializer::Toml(Toml {}),
                FsSerializer::Json => FileTypeSerializer::Json(Json {}),
            },
        }),
        DBEngine::Sqlite => {
            let mut sqlite = SqliteDb { conn: None };
            sqlite.init_db().await.unwrap();
            Box::new(sqlite)
        }
    }
}

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
    /// Ui
    #[command(alias = "u")]
    Ui,
    /// Print the completion script in stdout
    PrintCompleteScript { shell: Shell },
    /// Exec sql command (for debug purpose)
    Sql { q: String },
}

async fn run_wrapper(
    run_action_args: &mut Box<RunActionArgs>,
    v: Option<Verb>,
    db_handler: &dyn Db,
) {
    let requester = http::Api::new(run_action_args.timeout, run_action_args.insecure);
    if v.is_some() {
        run_action_args.verb = v.map(|v| v.to_string());
    }
    let _ = run_action_args
        .run_action(&requester, db_handler, true)
        .await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // init database if needed
    let db_handler = get_db().await;

    // parse cli args
    let mut cli: Cli = Cli::parse();

    match &mut cli.commands {
        Commands::Project(project) => match &mut project.project_commands {
            ProjectCommands::New(create_project_args) => {
                create_project_args.create(&*db_handler).await?;
            }
            ProjectCommands::RmAction(rm_action_args) => {
                rm_action_args.rm_action(&*db_handler).await?;
            }
            ProjectCommands::List(list_projects) => {
                list_projects.list_projects(&*db_handler).await?;
            }
            ProjectCommands::Info(project_info_args) => {
                project_info_args.show_info(&*db_handler).await?;
            }
        },
        Commands::Run(run) => match &mut run.run_commands {
            // init http requester
            RunCommands::Action(run_action_args) => {
                run_wrapper(run_action_args, None, &*db_handler).await;
            }
            RunCommands::TestSuite(test_suite_args) => {
                let requester = http::Api::new(Some(10), true);
                test_suite_args
                    .run_test_suite(&requester, &*db_handler)
                    .await?;
            }
            RunCommands::Get(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Get), &*db_handler).await;
            }
            RunCommands::Post(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Post), &*db_handler).await;
            }
            RunCommands::Put(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Put), &*db_handler).await;
            }
            RunCommands::Delete(run_action_args) => {
                run_wrapper(run_action_args, Some(Verb::Delete), &*db_handler).await;
            }
        },
        Commands::TestSuite(test_suite) => match &mut test_suite.ts_commands {
            TestSuiteCommands::New(create_test_suite_args) => {
                create_test_suite_args.upsert_test_suite(db_handler).await?;
            }
        },
        Commands::History(history) => match &mut history.history_commands {
            HistoryCommands::Ui => {
                let histories = db_handler.get_history(None).await?;
                let mut ui = commands::history::list_ui::HistoryUI::new(histories);
                ui.run_ui()?;
            }
            HistoryCommands::List(list_args) => {
                list_args.list_history(db_handler).await?;
            }
        },
        Commands::Ui => {
            let projects = db_handler.get_projects().await?;
            let mut ui = ui::app::App::new(projects, db_handler);
            ui.run_ui()?;
        }
        &mut Commands::PrintCompleteScript { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "apikrab".to_string(),
                &mut io::stdout(),
            );
        }
        Commands::Sql { q } => {
            let r = db_handler.get_connection().unwrap().fetch_many(q.as_str());
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
