use crate::commands::run::_test_checker::TestChecker;
use crate::db::db_handler::DBHandler;
use crate::db::dto::TestSuiteInstance;
use crate::http::Api;
use clap::Args;
use colored::Colorize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Args, Debug, Clone)]
pub struct TestSuiteArgs {
    /// Test suite name
    name: String,

    /// Debug output
    #[arg(short, long)]
    debug: bool,
}

impl TestSuiteArgs {
    pub async fn run_test_suite_instance(
        &self,
        api: &Api,
        db: &DBHandler,
        flow_name: &str,
        test: &TestSuiteInstance,
        ctx: &HashMap<String, String>,
    ) -> anyhow::Result<bool> {
        let flow = db.get_flow(flow_name).await?;
        let mut run_args = flow.de_run_action_args();
        run_args.force = true;
        run_args.quiet = !self.debug;
        let r = run_args.run_action(api, db).await;
        let value = r.expect("Error running flow...");
        let expected = serde_json::from_str::<HashMap<String, String>>(&test.expect)?;
        let is_success = TestChecker::new(&value, ctx, &expected).check(flow_name);
        Ok(is_success)
    }
    pub async fn run_test_suite(&self, api: &Api, db: &DBHandler) -> anyhow::Result<()> {
        let tests = db.get_test_suite_instance(&self.name).await?;
        let ctx = db.get_conf().await?.get_value();

        println!("Running test suite {}", self.name.green());
        let mut results: Vec<bool> = vec![];
        let p = indicatif::ProgressBar::new(tests.len() as u64);
        p.enable_steady_tick(Duration::from_millis(100));
        for test in tests {
            let is_success = self
                .run_test_suite_instance(api, db, &test.flow_name, &test, &ctx)
                .await?;
            results.push(is_success);
            p.inc(1);
        }
        p.finish_and_clear();
        if results.iter().all(|b| *b) {
            println!("{}", "🎉 All tests passed!".green());
        } else {
            println!("{}", "🔥 Some tests failed!".red());
        }
        Ok(())
    }
}
