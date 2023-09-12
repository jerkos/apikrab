use crate::db::db_handler::DBHandler;
use crate::db::dto::Action;
use clap::Args;

#[derive(Args)]
pub struct AddActionArgs {
    // project name
    pub project_name: String,

    // name of the action
    #[arg(short, long)]
    pub name: String,

    // url of the action
    #[arg(short, long)]
    pub url: String,

    // verb of the action
    #[arg(short, long)]
    pub verb: String,

    // maybe a static body
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
        db_handler.upsert_action(&action).await?;
        Ok(())
    }
}
