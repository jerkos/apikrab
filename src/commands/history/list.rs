use crate::db::db_trait::Db;
use clap::Args;

#[derive(Args)]
pub struct HistoryArgs {}

impl HistoryArgs {
    pub async fn list_history(&self, db_handler: Box<dyn Db>) -> anyhow::Result<()> {
        let history = db_handler.get_history(None).await?;
        history.iter().for_each(|h| println!("{}", h));
        Ok(())
    }
}
