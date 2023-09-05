mod commands;
mod db;
mod http;
mod json_path;
mod ui;
mod utils;

use crate::commands::history::{History, HistoryCommands};
use clap::{Args, Parser, Subcommand};

use crate::commands::project::{Project, ProjectCommands};
use crate::commands::run::{Run, RunCommands};
use crate::db::db_handler::DBHandler;
use crate::ui::run_ui::UIRunner;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create or update a new project with specified parameters
    Project(Project),
    // Add action to a specified url
    Run(Run),
    // history
    History(History),
}

#[derive(Args)]
struct DeleteActionArgs {
    name: String,
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
    match &mut cli.command {
        Commands::Project(project) => match &mut project.project_commands {
            ProjectCommands::Create(create_project_args) => {
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
        },
        Commands::Run(run) => match &mut run.run_commands {
            RunCommands::Action(run_action_args) => {
                run_action_args.run_action(&requester).await?;
            }
        },
        Commands::History(history) => match &mut history.history_commands {
            HistoryCommands::List(history_args) => {
                let histories = db_handler.get_history().await?;
                let mut ui = commands::history::list_ui::HistoryUI::new(histories);
                ui.run_ui()?;
            }
        },
    }
    Ok(())
}
