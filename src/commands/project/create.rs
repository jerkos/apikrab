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
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub from_openapi: Option<String>,

    /// url or path to postman collection file
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub from_postman: Option<String>,
}

impl CreateProjectArgs {
    pub fn get_importer<'a>(
        &'a self,
        db_handler: &'a DBHandler,
    ) -> Option<(Box<dyn Import + Sync + 'a>, &'a str)> {
        #[allow(clippy::manual_map)]
        if self.from_openapi.is_some() && self.from_postman.is_some() {
            return None;
        }
        if self.from_openapi.is_some() {
            return Some((
                Box::new(commands::import::openapi::OpenapiV3Importer { db_handler }),
                self.from_openapi.as_ref().unwrap(),
            ));
        }
        if self.from_postman.is_some() {
            return Some((
                Box::new(commands::import::postman::PostmanImporter { db_handler }),
                self.from_postman.as_ref().unwrap(),
            ));
        }
        None
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
