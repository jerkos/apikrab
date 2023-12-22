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
        api: &Api,
        db: &Box<dyn Db>,
        test: &TestSuiteInstance,
        multi_progress: &MultiProgress,
        pb: &indicatif::ProgressBar,
    ) -> anyhow::Result<bool> {
        let mut run_args = test.run_action_args.clone();
        run_args.force = true;
        run_args.quiet = !self.debug;
        // disable all saving !
        run_args.save = None;
        run_args.save_to_ts = None;
        let (_, r) = run_args
            .run_action(api, db, Some(multi_progress), Some(pb))
            .await;
        Ok(r.iter().all(|b| *b))
    }

    pub async fn run_test_suite(&self, api: &Api, db: &Box<dyn Db>) -> anyhow::Result<()> {
        let tests = db.get_test_suite_instance(&self.name).await?;

        println!("Running test suite {}", self.name.green());
        let mut results: Vec<bool> = vec![];

        let (multi, pb) = init_progress_bars(tests.len() as u64);
        // pb.enable_steady_tick(Duration::from_millis(100));

        for test in tests {
            let is_success = self
                .run_test_suite_instance(api, &db, &test, &multi, &pb)
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
