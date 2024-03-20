use crate::commands::project::create::CreateProjectArgs;
use crate::domain::DomainAction;
use crate::utils::parse_cli_conf_to_map;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct Project {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) main_url: String,
    pub(crate) conf: Option<HashMap<String, String>>,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

impl Project {
    pub fn get_project_conf(&self) -> anyhow::Result<HashMap<String, String>> {
        match self.conf {
            None => Ok(HashMap::new()),
            Some(ref conf) => Ok(conf.clone()),
        }
    }
}

impl FromRow<'_, SqliteRow> for Project {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Project {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            main_url: row.try_get("main_url")?,
            conf: serde_json::from_str(row.try_get("conf")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut conf_keys = self
            .get_project_conf()
            .expect("Error getting project conf")
            .keys()
            .map(String::from)
            .collect::<Vec<String>>()
            .join(",");
        if conf_keys.is_empty() {
            conf_keys = "N/A".to_string();
        }
        write!(
            f,
            "{}\n  conf: {}",
            self.name.bold().blue(),
            conf_keys.red()
        )
    }
}

impl From<&CreateProjectArgs> for Project {
    fn from(args: &CreateProjectArgs) -> Self {
        Project {
            id: None,
            name: args.name.clone(),
            main_url: args.url.clone(),
            conf: parse_cli_conf_to_map(args.conf.as_ref()),
            created_at: None,
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Action {
    pub(crate) id: Option<i64>,
    pub(crate) name: Option<String>,
    // chain actions
    pub(crate) actions: Vec<DomainAction>,

    // metadata
    pub(crate) body_example: Option<String>,
    pub(crate) response_example: Option<String>,
    // foreign key
    pub(crate) project_name: Option<String>,
    // chrono
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

impl Action {
    pub fn is_chained_action(&self) -> bool {
        self.actions.len() > 1
    }
}

impl FromRow<'_, SqliteRow> for Action {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Action {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            actions: serde_json::from_str(row.try_get("actions")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            body_example: serde_json::from_str(row.try_get("body_example")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            response_example: serde_json::from_str(row.try_get("response_example")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            project_name: row.try_get("project_name")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.name
                .as_ref()
                .map(|n| n.green())
                .unwrap_or("UNKNOWN".green()),
        )
    }
}

/// a Context for storing intermediates variables
/// when running a flow or several consecutive actions
/// Inserting a variable with a name which already exist
/// in the context overrides it.
#[derive(Clone, Debug)]
pub struct Context {
    /// value should be a json object
    pub value: HashMap<String, String>,
}

impl Context {
    pub fn get_value(&self) -> HashMap<String, String> {
        self.value.clone()
    }
}

impl FromRow<'_, SqliteRow> for Context {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Context {
            value: serde_json::from_str(row.try_get("value")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
        })
    }
}

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct History {
    pub(crate) id: Option<i64>,
    pub(crate) action_name: String,
    pub(crate) url: String,
    pub(crate) body: Option<Value>,
    pub(crate) headers: Option<String>,
    pub(crate) response: Option<String>,
    pub(crate) status_code: u16,
    pub(crate) duration: f32,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
}

impl Display for History {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | {} | {} | {:?}",
            self.created_at
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or("None".to_string())
                .cyan(),
            self.action_name.green(),
            self.status_code.to_string().yellow(),
            self.duration,
        )
    }
}

/// a test suite is a collection of flows
/// with expected results
#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct TestSuite {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
}

/// test suite instance are atomic flows with expectations
/// that are part of a test suite
#[derive(Serialize, Deserialize, Default)]
pub struct TestSuiteInstance {
    pub(crate) id: Option<i64>,
    pub(crate) test_suite_name: String,
    pub(crate) actions: Vec<DomainAction>,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

impl FromRow<'_, SqliteRow> for TestSuiteInstance {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(TestSuiteInstance {
            id: row.try_get("id")?,
            test_suite_name: row.try_get("test_suite_name")?,
            actions: serde_json::from_str(row.try_get("actions")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
