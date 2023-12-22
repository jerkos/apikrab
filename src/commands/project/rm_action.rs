use crate::db::db_trait::Db;
use clap::Args;

#[derive(Args)]
pub struct RmActionArgs {
    /// name of the action to remove
    #[arg(short, long)]
    name: String,

    /// name of the project to remove the action from
    #[arg(short, long)]
    project: String,
}

impl RmActionArgs {
    pub async fn rm_action(&self, db_handler: Box<dyn Db>) -> anyhow::Result<()> {
        db_handler
            .rm_action(&self.name, Some(self.project.as_str()))
            .await?;
        Ok(())
    }
}
