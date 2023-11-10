use crate::commands::project::add_action::AddActionArgs;
use crate::commands::project::create::CreateProjectArgs;
use crate::commands::run::action::RunActionArgs;
use crate::utils::parse_cli_conf_to_map;
use colored::Colorize;
use serde_json::to_string;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(sqlx::FromRow, Clone)]
pub struct Project {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) main_url: String,
    pub(crate) conf: Option<String>,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

impl Project {
    pub fn get_project_conf(&self) -> anyhow::Result<HashMap<String, String>> {
        match self.conf {
            None => Ok(HashMap::new()),
            Some(ref conf) => {
                let r = serde_json::from_str(conf);
                match r {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        anyhow::bail!(e)
                    }
                }
            }
        }
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
        let project_conf = parse_cli_conf_to_map(args.conf.as_ref());
        Project {
            id: None,
            name: args.name.clone(),
            main_url: args.url.clone(),
            conf: to_string(&project_conf).ok(),
            created_at: None,
            updated_at: None,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone, Default)]
pub struct Action {
    pub(crate) id: Option<i64>,
    pub(crate) name: Option<String>,
    // when basically add an action to a project
    // this can be empty
    pub(crate) run_action_args: Option<String>,
    // metadata
    pub(crate) body_example: Option<String>,
    pub(crate) response_example: Option<String>,
    // foreign key
    pub(crate) project_name: Option<String>,
    // chrono
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}

// todo use an intermediate struct to parse run_action_args
impl Action {
    pub fn get_run_action_args(&self) -> anyhow::Result<RunActionArgs> {
        match self.run_action_args {
            Some(ref run_action_args) => match serde_json::from_str(run_action_args) {
                Ok(r) => Ok(r),
                Err(e) => anyhow::bail!(e),
            },
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
#[derive(sqlx::FromRow)]
pub struct Context {
    /// value should be a json object
    pub value: String,
}

impl Context {
    pub fn get_value(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.value).expect("Error deserializing value")
    }
}

#[derive(sqlx::FromRow, Debug)]
pub struct History {
    pub(crate) id: Option<i64>,
    pub(crate) action_name: String,
    pub(crate) url: String,
    pub(crate) body: Option<String>,
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
#[derive(sqlx::FromRow)]
pub struct TestSuite {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
}

/// test suite instance are atomic flows with expectations
/// that are part of a test suite
#[derive(sqlx::FromRow)]
pub struct TestSuiteInstance {
    pub(crate) id: Option<i64>,
    pub(crate) test_suite_name: String,
    pub(crate) run_action_args: String,
    pub(crate) created_at: Option<chrono::NaiveDateTime>,
    pub(crate) updated_at: Option<chrono::NaiveDateTime>,
}
