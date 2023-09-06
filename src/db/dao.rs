use crate::commands::project::add_action::AddActionArgs;
use crate::commands::project::create::CreateProjectArgs;
use crate::utils::parse_conf_to_map;
use colored::Colorize;
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
            Some(url) => url,
            None => "None",
        };
        let prod_url = match &self.prod_url {
            Some(url) => url,
            None => "None",
        };
        write!(
            f,
            "Project: {}\nTest URL: {}\nProd URL: {}\nConf: {}",
            self.name, test_url, prod_url, self.conf
        )
    }
}

impl From<&CreateProjectArgs> for Project {
    fn from(args: &CreateProjectArgs) -> Self {
        let conf = parse_conf_to_map(&args.conf);
        let conf_as_str = serde_json::to_string(&conf).expect("Error serializing conf");
        Project {
            name: args.name.clone(),
            test_url: args.test_url.clone(),
            prod_url: args.prod_url.clone(),
            conf: conf_as_str,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct LightAction {
    pub name: String,
    pub url: String,
    pub verb: String,
    pub static_body: Option<String>,
    pub headers: String,
    pub body_example: Option<String>,
    pub response_example: Option<String>,
    pub project_name: String,
}

impl LightAction {
    pub fn headers_as_map(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.headers).expect("Error deserializing headers")
    }
}

impl From<&AddActionArgs> for LightAction {
    fn from(value: &AddActionArgs) -> Self {
        let mut headers = parse_conf_to_map(&value.header);
        if value.form {
            headers.insert(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            );
        }
        let headers_as_str = serde_json::to_string(&headers).expect("Error serializing headers");
        LightAction {
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

impl Display for LightAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.verb.cyan(),
            self.url.yellow(),
            self.name.green(),
            self.headers
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
