use crate::commands::import::openapi::Import;
use crate::db::db_handler::DBHandler;
use crate::{commands, db};
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

    /// url or path to openapi file
    #[arg(short, long)]
    pub from_openapi: Option<String>,
}

impl CreateProjectArgs {
    pub fn get_importer<'a>(
        &'a self,
        db_handler: &'a DBHandler,
    ) -> Option<(impl Import + 'a, &'a str)> {
        match &self.from_openapi {
            Some(path) => Some((
                commands::import::openapi::OpenapiV3Importer { db_handler },
                path,
            )),
            None => None,
        }
    }
    pub async fn create(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        match self.get_importer(db_handler) {
            Some((importer, path)) => {
                let mut project = self.into();
                let content = importer.load(path).await?;
                importer.import(&content, &mut project).await?;
            }
            None => {
                let project: db::dto::Project = self.into();
                db_handler.upsert_project(&project).await?;
            }
        }
        Ok(())
    }
}
