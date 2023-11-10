use crate::db::{db_handler::DBHandler, dto::TestSuite};
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct CreateTestSuiteArgs {
    /// Test suite name
    name: String,
}

impl CreateTestSuiteArgs {
    pub async fn upsert_test_suite(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        let test_suite = TestSuite {
            id: None,
            name: self.name.clone(),
            created_at: None,
        };
        db_handler.upsert_test_suite(&test_suite).await
    }
}
