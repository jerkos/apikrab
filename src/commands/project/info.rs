use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct ProjectInfoArgs {
    // Project name
    name: String,
}

impl ProjectInfoArgs {
    pub async fn show_info(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        db_handler.list_project_data(&self.name).await
    }
}
