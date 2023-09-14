use crate::db::db_handler::DBHandler;
use clap::Args;
use colored::Colorize;

#[derive(Args)]
pub struct ProjectInfoArgs {
    // Project name
    name: String,
}

impl ProjectInfoArgs {
    pub async fn show_info(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let project = db_handler.get_project(&self.name).await?;
        println!("{}\n", project.name.blue().bold());

        let actions = db_handler.get_actions(&self.name).await?;
        actions.iter().for_each(|action| println!("   {}", action));
        Ok(())
    }
}
