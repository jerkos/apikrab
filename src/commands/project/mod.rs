pub mod add_action;
pub mod create;
pub mod info;
pub mod list;
mod rm_action;

//pub mod project {
use crate::commands::project::add_action::AddActionArgs;
use crate::commands::project::create::CreateProjectArgs;
use crate::commands::project::info::ProjectInfoArgs;
use crate::commands::project::list::ListProjects;
use crate::commands::project::rm_action::RmActionArgs;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Project {
    #[command(subcommand)]
    pub project_commands: ProjectCommands,
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Create a new project
    Create(CreateProjectArgs),
    // Add action
    AddAction(AddActionArgs),
    RmAction(RmActionArgs),
    // Get information about a project
    Info(ProjectInfoArgs),
    // list projects
    List(ListProjects),
}
