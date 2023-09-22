use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args)]
pub struct FlowListArgs {}

impl FlowListArgs {
    pub async fn list_flows(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let flows = db_handler.get_flows().await?;
        flows
            .iter()
            .enumerate()
            .for_each(|(i, h)| println!("{}. {}", i + 1, h));
        Ok(())
    }
}
