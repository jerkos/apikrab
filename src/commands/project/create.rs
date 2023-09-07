use crate::db;
use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct CreateProjectArgs {
    /// project name unique
    pub name: String,

    /// test url for this project
    #[arg(short, long)]
    pub test_url: Option<String>,

    /// prod url for this project
    #[arg(short, long)]
    pub prod_url: Option<String>,

    /// Possible configuration for this project
    #[arg(short, long)]
    pub conf: Option<Vec<String>>,
}

impl CreateProjectArgs {
    pub async fn create(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let project: db::dto::Project = self.into();
        db_handler.upsert_project(&project).await?;
        Ok(())
    }
}
