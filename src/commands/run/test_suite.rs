use std::collections::HashMap;

use crate::commands::run::_printer::Printer;
use crate::commands::run::_progress_bar::init_progress_bars;
use crate::db::db_trait::Db;
use crate::db::dto::TestSuiteInstance;
use crate::http::Api;
use clap::Args;
use colored::Colorize;
use indicatif::MultiProgress;

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
        test: &TestSuiteInstance,
        api: &Api,
        db: &Box<dyn Db>,
        printer: &mut Printer,
        multi_progress: &MultiProgress,
        pb: &indicatif::ProgressBar,
    ) -> anyhow::Result<bool> {
        let mut ctx = HashMap::new();
        for action in &test.actions {
            let r = action
                .run_with_tests(None, &mut ctx, db, api, printer, multi_progress, pb)
                .await;
            // disable all saving !
            if !r.iter().all(|b| *b) {
                println!(
                    "Test suite {} failed on action {}",
                    self.name.red(),
                    action.name.red()
                );
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn run_test_suite(&self, api: &Api, db: &Box<dyn Db>) -> anyhow::Result<()> {
        let tests = db.get_test_suite_instance(&self.name).await?;

        println!("Running test suite {}", self.name.green());
        let mut results: Vec<bool> = vec![];

        let (multi, pb) = init_progress_bars(tests.len() as u64);

        // create a global printer
        let mut printer = Printer::new(!self.debug, false, false);

        for test in tests {
            // create a ctx for each test
            let is_success = self
                .run_test_suite_instance(&test, api, db, &mut printer, &multi, &pb)
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
