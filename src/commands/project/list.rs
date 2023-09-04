use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct ListProjects {}

impl ListProjects {
    pub async fn list_projects(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        db_handler.list_projects().await
    }
}
