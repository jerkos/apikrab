use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct ListProjects {}

impl ListProjects {
    pub async fn list_projects(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let projects = db_handler.get_projects().await?;
        projects
            .iter()
            .for_each(|project| println!("{}", project.to_string()));
        Ok(())
    }
}
