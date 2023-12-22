use async_trait::async_trait;
use tokio::fs;

use crate::{
    db::dto::{Action, History, TestSuite, TestSuiteInstance},
    HOME_DIR,
};

use super::{
    db_trait::Db,
    dto::{Context, Project},
};

#[derive(Clone, Default)]
pub struct JsonHandler {
    pub root: Option<String>,
}

impl JsonHandler {
    fn get_root(&self) -> String {
        self.root
            .as_ref()
            .map(String::to_string)
            .unwrap_or_else(|| format!("{}/.config/qapi", HOME_DIR.display()))
    }
}

#[async_trait]
impl Db for JsonHandler {
    fn box_clone(&self) -> Box<dyn Db> {
        Box::new(self.clone())
    }

    /// Return current configuration as key value from file conf.json
    /// in the root folder.
    async fn get_conf(&self) -> anyhow::Result<Context> {
        Ok(Context {
            value: serde_json::from_str(
                &fs::read_to_string(format!("{}/conf.json", self.get_root())).await?,
            )?,
        })
    }

    /// Overwrite the configuration file with the given context.
    async fn insert_conf(&self, context: &Context) -> anyhow::Result<i64> {
        fs::write(
            format!("{}/conf.json", self.get_root()),
            serde_json::to_string_pretty(&context.value)?,
        )
        .await?;
        Ok(0)
    }

    /// Insert a new history entry in the history folder.
    /// No op for the moment. Requires appending to a file.
    /// Would be great if we do not need to parse the file.
    async fn insert_history(&self, _history: &History) -> anyhow::Result<i64> {
        Ok(0)
    }

    /// Return all history entries from the history folder.
    async fn get_history(&self, _limit: Option<u16>) -> anyhow::Result<Vec<History>> {
        Ok(vec![])
    }

    /// Return all actions from a project or all actions if no project name is given.
    async fn get_actions(&self, project_name: Option<&str>) -> anyhow::Result<Vec<Action>> {
        let mut actions = vec![];
        let dirname = match project_name.as_ref() {
            Some(p_name) => format!("{}/projects/{}", self.get_root(), p_name),
            None => format!("{}/projects/default", self.get_root()),
        };
        let mut dir = fs::read_dir(dirname).await?;
        while let Some(entry) = dir.next_entry().await? {
            actions.push(serde_json::from_str(
                &fs::read_to_string(entry.path()).await?,
            )?);
        }
        Ok(actions)
    }

    /// Insert a new action in the project folder.
    async fn upsert_action(&self, action: &Action) -> anyhow::Result<()> {
        let dirname = match action.project_name.as_ref() {
            Some(p_name) => format!("{}/{}", self.get_root(), p_name),
            None => format!("{}/projects/default", self.get_root()),
        };

        fs::create_dir_all(&dirname).await?;

        fs::write(
            format!(
                "{}/{}.json",
                dirname,
                action.name.as_ref().expect("Action name is required")
            ),
            serde_json::to_string_pretty(&action)?,
        )
        .await?;
        Ok(())
    }

    /// Return an action from the project folder.
    async fn get_action(&self, action_name: &str, project: Option<&str>) -> anyhow::Result<Action> {
        let dirname = match project.as_ref() {
            Some(p_name) => format!("{}/{}", self.get_root(), p_name),
            None => format!("{}/projects/default", self.get_root()),
        };

        serde_json::from_str::<Action>(
            &fs::read_to_string(format!("{}/{}.json", dirname, action_name)).await?,
        )
        .map_err(|e: serde_json::Error| anyhow::anyhow!("Error parsing action: {}", e))
    }

    async fn rm_action(&self, action_name: &str, project: Option<&str>) -> anyhow::Result<u64> {
        let dirname = match project.as_ref() {
            Some(p_name) => format!("{}/{}", self.get_root(), p_name),
            None => format!("{}/projects/default", self.get_root()),
        };

        fs::remove_file(format!("{}/{}.json", dirname, action_name)).await?;
        Ok(1)
    }

    async fn upsert_test_suite(&self, test_suite: &TestSuite) -> anyhow::Result<()> {
        // create test suite folder do nothing if it exists
        fs::create_dir_all(format!(
            "{}/test-suites/{}",
            self.get_root(),
            test_suite.name
        ))
        .await?;
        Ok(())
    }

    async fn upsert_test_suite_instance(
        &self,
        test_suite_instance: &TestSuiteInstance,
    ) -> anyhow::Result<()> {
        let mut test_suite_id = test_suite_instance.id.unwrap_or(0);
        if test_suite_id == 0 {
            let mut dir = fs::read_dir(format!(
                "{}/test-suites/{}",
                self.get_root(),
                test_suite_instance.test_suite_name
            ))
            .await?;
            while let Some(entry) = dir.next_entry().await? {
                let file_name = entry.file_name().into_string().unwrap();
                let id = file_name.split('.').next().unwrap().parse::<i64>()?;
                if id > test_suite_id {
                    test_suite_id = id;
                }
            }
            test_suite_id += 1;
        }

        fs::write(
            format!(
                "{}/test-suites/{}/{}.json",
                self.get_root(),
                test_suite_instance.test_suite_name.clone(),
                test_suite_id
            ),
            serde_json::to_string_pretty(&test_suite_instance)?,
        )
        .await?;

        Ok(())
    }

    async fn get_test_suite_instance(
        &self,
        test_suite_name: &str,
    ) -> anyhow::Result<Vec<TestSuiteInstance>> {
        let mut instances = vec![];
        let mut dir = fs::read_dir(format!(
            "{}/test-suites/{}",
            self.get_root(),
            test_suite_name
        ))
        .await?;
        while let Some(entry) = dir.next_entry().await? {
            instances.push(serde_json::from_str(
                &fs::read_to_string(entry.path()).await?,
            )?);
        }
        Ok(instances)
    }

    async fn get_project(&self, project_name: &str) -> anyhow::Result<Project> {
        Ok(Project {
            name: project_name.to_string(),
            ..Default::default()
        })
    }

    async fn upsert_project(&self, project: &Project) -> anyhow::Result<i64> {
        fs::create_dir_all(format!("{}/projects/{}", self.get_root(), project.name)).await?;
        Ok(0)
    }

    async fn get_projects(&self) -> anyhow::Result<Vec<Project>> {
        let mut projects = vec![];
        let mut dir = fs::read_dir(format!("{}/projects", self.get_root())).await?;
        while let Some(entry) = dir.next_entry().await? {
            let file_name = entry.file_name().into_string().unwrap();
            let project_name = file_name.split('.').next().unwrap();
            projects.push(Project {
                name: project_name.to_string(),
                ..Default::default()
            });
        }
        Ok(projects)
    }
}
