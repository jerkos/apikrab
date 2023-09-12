use crate::db::db_handler::DBHandler;
use crate::db::dto::TestSuiteInstance;
use crate::utils::parse_multiple_conf;
use clap::Args;
use std::collections::HashMap;

#[derive(Args, Debug, Clone)]
pub struct AddTestSuiteArgs {
    /// Test suite name
    name: String,

    /// Flow name to add to the test suite
    #[arg(short, long)]
    flow_name: String,

    /// expect associated to the test
    #[arg(short, long)]
    expect: Vec<String>,
}

impl AddTestSuiteArgs {
    pub async fn add_test_suite(&self, db_handler: &DBHandler) -> anyhow::Result<()> {
        println!("Adding test suite {} to flow {}", self.name, self.flow_name);
        let vec_as_str = self.expect.join(",");
        let expected =
            serde_json::to_string::<HashMap<String, String>>(&parse_multiple_conf(&vec_as_str))
                .expect("Error serializing conf");
        let test_suite_instance = TestSuiteInstance {
            test_suite_name: self.name.clone(),
            flow_name: self.flow_name.clone(),
            expect: expected,
        };
        db_handler
            .upsert_test_suite_instance(&test_suite_instance)
            .await?;
        Ok(())
    }
}