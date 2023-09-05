use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct HistoryArgs {}

impl HistoryArgs {
    pub async fn list_history(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let history = db_handler.get_history().await?;
        history.iter().for_each(|h| println!("{}", h));
        Ok(())
    }
}
