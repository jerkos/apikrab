use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct RmActionArgs {
    /// name of the action to remove
    #[arg(short, long)]
    name: String,
}

impl RmActionArgs {
    pub async fn rm_action(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        db_handler.rm_action(&self.name).await?;
        Ok(())
    }
}
