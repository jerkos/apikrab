use crate::commands::run::_progress_bar::init_progress_bars;
use crate::commands::run::_test_checker::TestChecker;
use crate::db::db_handler::DBHandler;
use crate::db::dto::TestSuiteInstance;
use crate::http::Api;
use clap::Args;
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json::from_str;
use std::collections::HashMap;
use std::time::Duration;

use super::action::RunActionArgs;

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
        test: &TestSuiteInstance,
        ctx: &HashMap<String, String>,
        multi_progress: &MultiProgress,
    ) -> anyhow::Result<bool> {
        let mut run_args = from_str::<RunActionArgs>(&test.run_action_args)?;
        run_args.force = true;
        run_args.quiet = !self.debug;
        // disable all saving !
        run_args.save = None;
        run_args.save_to_ts = None;
        let (_, r) = run_args.run_action(api, db, Some(multi_progress)).await;
        Ok(r.iter().all(|b| *b))
    }

    pub async fn run_test_suite(&self, api: &Api, db: &DBHandler) -> anyhow::Result<()> {
        let tests = db.get_test_suite_instance(&self.name).await?;
        let ctx = db
            .get_conf()
            .await
            .map(|c| c.get_value())
            .unwrap_or(HashMap::new());

        println!("Running test suite {}", self.name.green());
        let mut results: Vec<bool> = vec![];

        let (multi, pb) = init_progress_bars(tests.len() as u64);
        // pb.enable_steady_tick(Duration::from_millis(100));

        for test in tests {
            let is_success = self
                .run_test_suite_instance(api, db, &test, &ctx, &multi)
                .await?;
            pb.inc(1);
            results.push(is_success);
        }
        pb.finish_with_message("DONE");

        if results.iter().all(|b| *b) {
            println!("{}", "🎉 All tests passed!".green());
        } else {
            println!("{}", "🔥 Some tests failed!".red());
        }
        Ok(())
    }
}
