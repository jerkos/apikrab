use async_trait::async_trait;
use sqlx::SqlitePool;

use super::dto::{Action, Context, History, Project, TestSuite, TestSuiteInstance};

#[async_trait]
pub trait Db: Send + Sync {
    fn box_clone(&self) -> Box<dyn Db>;

    fn get_connection(&self) -> Option<&SqlitePool> {
        None
    }
    async fn get_conf(&self) -> anyhow::Result<Context>;
    async fn insert_conf(&self, context: &Context) -> anyhow::Result<i64>;
    async fn upsert_action(&self, action: &Action) -> anyhow::Result<()>;
    async fn insert_history(&self, history: &History) -> anyhow::Result<i64>;
    async fn get_history(&self, limit: Option<u16>) -> anyhow::Result<Vec<History>>;

    async fn get_action(&self, action_name: &str, project: Option<&str>) -> anyhow::Result<Action>;
    async fn get_actions(&self, project_name: Option<&str>) -> anyhow::Result<Vec<Action>>;
    async fn rm_action(&self, action_name: &str, project: Option<&str>) -> anyhow::Result<u64>;
    async fn upsert_test_suite(&self, test_suite: &TestSuite) -> anyhow::Result<()>;
    async fn upsert_test_suite_instance(
        &self,
        test_suite_instance: &TestSuiteInstance,
    ) -> anyhow::Result<()>;
    async fn get_test_suite_instance(
        &self,
        test_suite_name: &str,
    ) -> anyhow::Result<Vec<TestSuiteInstance>>;

    async fn get_project(&self, project_name: &str) -> anyhow::Result<Project>;
    async fn upsert_project(&self, project: &Project) -> anyhow::Result<i64>;
    async fn get_projects(&self) -> anyhow::Result<Vec<Project>>;
}

impl Clone for Box<dyn Db> {
    fn clone(&self) -> Box<dyn Db> {
        self.box_clone()
    }
}
