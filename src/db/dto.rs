use crate::commands::project::add_action::AddActionArgs;
use crate::commands::project::create::CreateProjectArgs;
use crate::commands::run::action::RunActionArgs;
use crate::utils::parse_cli_conf_to_map;
use colored::Colorize;
use itertools::EitherOrBoth::Both;
use itertools::{EitherOrBoth, Itertools};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(sqlx::FromRow, Clone)]
pub struct Project {
    pub name: String,
    pub test_url: Option<String>,
    pub prod_url: Option<String>,
    pub conf: String,
}

impl Project {
    pub fn get_conf(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.conf).expect("Error deserializing conf")
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let test_url = match &self.test_url {
            Some(url) => url.yellow(),
            None => "N/A".to_string().yellow(),
        };
        let prod_url = match &self.prod_url {
            Some(url) => url.yellow(),
            None => "N/A".to_string().yellow(),
        };
        let mut conf_keys = self
            .get_conf()
            .keys()
            .map(String::from)
            .collect::<Vec<String>>()
            .join(",");
        if conf_keys.is_empty() {
            conf_keys = "N/A".to_string();
        }
        write!(
            f,
            "{}\n   test: {} prod: {}\n   conf: {}",
            self.name.bold().blue(),
            test_url,
            prod_url,
            conf_keys.red()
        )
    }
}

impl From<&CreateProjectArgs> for Project {
    fn from(args: &CreateProjectArgs) -> Self {
        let conf = parse_cli_conf_to_map(&args.conf);
        let conf_as_str = serde_json::to_string(&conf).expect("Error serializing conf");
        Project {
            name: args.name.clone(),
            test_url: args.test_url.clone(),
            prod_url: args.prod_url.clone(),
            conf: conf_as_str,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone, Default)]
pub struct Action {
    pub name: String,
    pub url: String,
    pub verb: String,
    pub static_body: Option<String>,
    pub headers: String,
    pub body_example: Option<String>,
    pub response_example: Option<String>,
    pub project_name: String,
}

impl Action {
    pub fn headers_as_map(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.headers).expect("Error deserializing headers")
    }

    pub fn is_form(&self) -> bool {
        self.headers_as_map()
            .get(reqwest::header::CONTENT_TYPE.as_str())
            .unwrap_or(&"".to_string())
            == "application/x-www-form-urlencoded"
    }
}

impl From<&AddActionArgs> for Action {
    fn from(value: &AddActionArgs) -> Self {
        let mut headers = parse_cli_conf_to_map(&value.header);
        if value.form {
            headers.insert(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            );
        }
        let headers_as_str = serde_json::to_string(&headers).expect("Error serializing headers");
        Action {
            name: value.name.clone(),
            url: value.url.clone(),
            verb: value.verb.clone(),
            static_body: value.static_body.clone(),
            headers: headers_as_str,
            body_example: None,
            response_example: None,
            project_name: value.project_name.clone(),
        }
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.name.green(),
            self.verb.cyan(),
            self.url.yellow(),
            self.headers_as_map()
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(", ")
                .bold()
                .blue(),
        )
    }
}

#[derive(sqlx::FromRow)]
pub struct Context {
    pub value: String,
}

impl Context {
    pub fn get_value(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.value).expect("Error deserializing value")
    }
}

#[derive(sqlx::FromRow, Debug)]
pub struct History {
    pub id: Option<i64>,
    pub action_name: String,
    pub url: String,
    pub body: Option<String>,
    pub headers: Option<String>,
    pub response: Option<String>,
    pub status_code: u16,
    pub duration: f32,
    pub timestamp: Option<chrono::NaiveDateTime>,
}

impl Display for History {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | {} | {} | {:?}",
            self.timestamp
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or("None".to_string())
                .cyan(),
            self.action_name.green(),
            self.status_code.to_string().yellow(),
            self.duration,
        )
    }
}

#[derive(sqlx::FromRow)]
pub struct Flow {
    pub name: String,
    pub run_action_args: String,
}

impl Flow {
    pub fn de_run_action_args(&self) -> RunActionArgs {
        serde_json::from_str::<RunActionArgs>(self.run_action_args.as_str()).unwrap()
    }
}

impl Display for Flow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ac = self.de_run_action_args();
        let action = ac.name;
        let mut chain = ac
            .chain
            .unwrap_or(vec![])
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<String>>();
        chain.insert(0, action);

        let extracted = ac.extract_path.unwrap_or(vec![]);

        write!(f, "{}\n   ", self.name.green()).unwrap();
        let chain_len = chain.len();
        let mut i = 0;
        chain.iter().zip_longest(extracted.iter()).for_each(|val| {
            match val {
                Both(a, e) => {
                    write!(f, "{}({})", a.yellow(), e.blue()).unwrap();
                }
                EitherOrBoth::Left(a) => {
                    write!(f, "{}", a.yellow()).unwrap();
                }
                EitherOrBoth::Right(e) => {
                    write!(f, "{}", e.blue()).unwrap();
                }
            }
            i += 1;
            if i < chain_len {
                write!(f, " -> ").unwrap();
            }
        });
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
pub struct TestSuite {
    pub name: String,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(sqlx::FromRow)]
pub struct TestSuiteInstance {
    pub test_suite_name: String,
    pub flow_name: String,
    pub expect: String,
}
