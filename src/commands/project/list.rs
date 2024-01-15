use crate::db::db_trait::Db;
use clap::Args;

#[derive(Args)]
pub struct ListProjects {}

impl ListProjects {
    pub async fn list_projects(&self, db_handler: &dyn Db) -> anyhow::Result<()> {
        let projects = db_handler.get_projects().await?;
        projects.iter().enumerate().for_each(|(i, h)| {
            println!("{} - {}", i + 1, h);
        });
        Ok(())
    }
}
