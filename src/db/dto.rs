use crate::commands::project::add_action::AddActionArgs;
use crate::commands::project::create::CreateProjectArgs;
use crate::commands::run::action::RunActionArgs;
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
    // when basically add an action to a project
    // this can be empty
    pub(crate) run_action_args: Option<RunActionArgs>,
    // metadata
    pub(crate) body_example: Option<Value>,
    pub(crate) response_example: Option<Value>,
    // foreign key
    pub(crate) project_name: Option<String>,
    // chrono
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

// todo use an intermediate struct to parse run_action_args
impl Action {
    pub fn get_run_action_args(&self) -> anyhow::Result<RunActionArgs> {
        match self.run_action_args.as_ref() {
            Some(raa) => Ok(raa.clone()),
            None => Err(anyhow::anyhow!("No run action args for action")),
        }
    }
    // try parse headers as a map
    pub fn get_headers(&self) -> anyhow::Result<HashMap<String, String>> {
        match self.get_run_action_args() {
            Ok(r) => Ok(parse_cli_conf_to_map(r.header.as_ref()).unwrap()),
            Err(e) => anyhow::bail!(e),
        }
    }
}

impl FromRow<'_, SqliteRow> for Action {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Action {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            run_action_args: serde_json::from_str(row.try_get("run_action_args")?)
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

impl From<&AddActionArgs> for Action {
    fn from(add_action_args: &AddActionArgs) -> Self {
        // panicking if both are defined
        if add_action_args.url_encoded && add_action_args.form_data {
            panic!("Cannot have both url encoded and form data");
        }

        Action {
            // action is none because it's not in db yet
            id: None,
            name: Some(add_action_args.name.clone()),
            run_action_args: None,
            body_example: None,
            response_example: None,
            project_name: Some(add_action_args.project_name.clone()),
            created_at: None,
            updated_at: None,
        }
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let run_actions_args = self
            .get_run_action_args()
            .expect("Error getting run action args");
        write!(
            f,
            "{} {} {} {}",
            run_actions_args
                .name
                .map(|n| n.green())
                .unwrap_or("UNKNOWN".green()),
            run_actions_args
                .verb
                .map(|v| v.cyan())
                .unwrap_or("default".cyan()),
            run_actions_args
                .url
                .map(|u| u.yellow())
                .unwrap_or("default".yellow()),
            self.get_headers()
                .unwrap_or_else(|_| HashMap::new())
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(", ")
                .bold()
                .blue(),
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
    pub(crate) run_action_args: RunActionArgs,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

impl FromRow<'_, SqliteRow> for TestSuiteInstance {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(TestSuiteInstance {
            id: row.try_get("id")?,
            test_suite_name: row.try_get("test_suite_name")?,
            run_action_args: serde_json::from_str(row.try_get("run_action_args")?)
                .map_err(|e| sqlx::Error::Decode(e.into()))?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
