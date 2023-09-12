use crate::db::db_handler::DBHandler;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct CreateTestSuiteArgs {
    /// Test suite name
    name: String,
}

impl CreateTestSuiteArgs {
    pub async fn upsert_test_suite(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        db_handler.upsert_test_suite(&self.name).await
    }
}
