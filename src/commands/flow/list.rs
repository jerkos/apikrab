use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct FlowListArgs {}

impl FlowListArgs {
    pub async fn list_flows(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let flows = db_handler.get_flows().await?;
        flows.iter().for_each(|h| println!("{}", h));
        Ok(())
    }
}
