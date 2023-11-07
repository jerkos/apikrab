use crate::commands::run::action::RunActionArgs;
use crate::db::db_handler::DBHandler;
use crate::db::dto::Action;
use clap::Args;
use crossterm::style::Stylize;

#[derive(Args)]
pub struct AddActionArgs {
    /// project name
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
    pub static_body: Option<Vec<String>>,

    /// adding header separated by a :
    #[arg(short = 'H', long)]
    pub header: Option<Vec<String>>,

    /// shortcut to add a form encoded header
    #[arg(long)]
    pub form_data: bool,

    /// url encoded body
    #[arg(long)]
    pub url_encoded: bool,
}

impl AddActionArgs {
    pub async fn add_action(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let mut action: Action = self.into();
        action.project_name = Some(self.project_name.clone());

        let mut r = RunActionArgs::default();
        r.name = Some(self.name.clone());
        r.url = Some(self.url.clone());
        r.verb = Some(self.verb.clone());
        r.header = self.header.clone();
        r.body = self.static_body.clone();
        r.form_data = self.form_data;
        r.url_encoded = self.url_encoded;

        action.run_action_args = Some(serde_json::to_string(&r)?);

        let r = db_handler.upsert_action(&action).await;
        match r {
            Ok(_) => println!(
                "{}",
                format!("{}", format!("Action {} saved", self.name.clone().green()))
            ),
            Err(e) => println!("{}", format!("Error saving flow {}", e)),
        }
        Ok(())
    }
}
