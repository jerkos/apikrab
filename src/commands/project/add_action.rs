use crate::complete::complete_update;
use crate::db::db_handler::DBHandler;
use crate::db::dto::Action;
use crate::PROJECTS;
use clap::builder::PossibleValuesParser;
use clap::Args;

#[derive(Args)]
pub struct AddActionArgs {
    /// project name
    #[arg(value_parser = PossibleValuesParser::new(PROJECTS.as_slice()))]
    pub project_name: String,

    /// name of the action
    #[arg(short, long)]
    pub name: String,

    /// url of the action
    #[arg(short, long)]
    pub url: String,

    /// verb of the action
    #[arg(short, long, value_parser = ["GET", "POST", "PUT", "DELETE"])]
    pub verb: String,

    /// maybe a static body
    #[arg(short, long)]
    pub static_body: Option<String>,

    // adding header separated by a :
    #[arg(long)]
    pub header: Option<Vec<String>>,

    // shortcut to add a form encoded header
    #[arg(short, long)]
    pub form: bool,
}

impl AddActionArgs {
    pub async fn add_action(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let action: Action = self.into();

        db_handler.upsert_action(&action, false).await?;
        complete_update(db_handler.conn.as_ref().unwrap()).await?;
        Ok(())
    }
}
