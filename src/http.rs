use std::collections::HashMap;
use std::time::{Duration, Instant};

use colored::Colorize;
use log::debug;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::db::dto::{Context, History, Action};
use crate::db::db_handler::DBHandler;
use crate::{json_path, utils::replace_with_conf};

pub struct Api<'a> {
    client: reqwest::Client,
    pub db_handler: &'a DBHandler,
}

impl<'a> Api<'a> {
    pub fn new(db_handler: &'a DBHandler) -> Self {
        Self {
            client: reqwest::Client::new(),
            db_handler,
        }
    }

    pub fn _parse_conf(conf: &str) -> HashMap<String, String> {
        serde_json::from_str::<HashMap<String, String>>(conf).expect("Invalid conf")
    }

    pub fn interpolate_body_from_cli(
        &self,
        cli_body: &Option<String>,
        conf: &HashMap<String, String>,
    ) -> Option<String> {
        cli_body
            .as_ref()
            .map(|body| replace_with_conf(body, conf))
            .or(cli_body.to_owned())
    }

    pub fn interpolate_hashmap(
        &self,
        cli_headers: &HashMap<String, String>,
        conf: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        cli_headers
            .iter()
            .map(|(k, v)| {
                let computed_value = replace_with_conf(v, conf);
                (k.to_string(), computed_value.to_string())
            })
            .collect::<HashMap<String, String>>()
    }

    fn interpolate_path_params(
        &self,
        action: &Action,
        project: &crate::db::dto::Project,
        params: &Option<HashMap<String, String>>,
        ctx: &HashMap<String, String>,
    ) -> String {
        // getting an url like /api/v1/users/{id} and a hashmap like {id: 1}
        // next check in query params if exists then in conf

        let full_url = format!("{}/{}", project.test_url.as_ref().unwrap(), action.url);
        params
            .as_ref()
            .map(|path_params| replace_with_conf(&full_url, &path_params))
            .map(|full_url| replace_with_conf(&full_url, ctx))
            .unwrap_or_else(|| replace_with_conf(&full_url, ctx))
    }

    fn print_response(&self, response: &str) -> anyhow::Result<()> {
        println!("Received response: ");
        let response_as_value = serde_json::from_str::<serde_json::Value>(&response)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&response_as_value)?
                .split("\n")
                .take(10)
                .collect::<Vec<&str>>()
                .join("\n")
        );
        println!("...");
        Ok(())
    }

    fn extract_pattern(
        &self,
        (pattern_to_extract, value_name): (&str, Option<&str>),
        response: &str,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<bool> {
        let extracted = json_path::json_path(&response, pattern_to_extract);

        let extracted_as_string = extracted
            .map(|value| match serde_json::to_string_pretty(&value) {
                Ok(v) => {
                    if v.starts_with("\"") {
                        v[1..v.len() - 1].to_owned()
                    } else {
                        v
                    }
                }
                Err(_) => "".to_string(),
            })
            .unwrap_or("".to_string());

        if extracted_as_string.is_empty() {
            return Ok(false);
        }

        println!(
            "Extraction of {}: {} {}",
            pattern_to_extract.bright_green(),
            extracted_as_string.bright_magenta(),
            value_name
                .map(|v| format!("saved as {}", v.bright_yellow()))
                .unwrap_or("".to_string())
        );

        match value_name {
            Some(value_name) => {
                if value_name.is_empty() {
                    return Ok(false);
                }
                ctx.insert(value_name.to_string(), extracted_as_string);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub async fn handle_result(
        &self,
        light_action: &mut Action,
        body: &Option<String>,
        result: &anyhow::Result<(String, u16, Duration)>,
        extract_pattern: &Option<HashMap<String, Option<String>>>,
        no_print: bool,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        match result {
            Ok((response, status, _)) => {
                let status_code = format!("Status code: {}", status);
                if status >= &400 {
                    println!("{}", status_code.bold().red());
                    if !no_print {
                        println!("{}", response);
                    }
                    return Ok(());
                }

                // Successful request
                println!("{}", status_code.bold().green());

                light_action.response_example = Some(response.clone());
                light_action.body_example = body.clone();
                self.db_handler.upsert_action(&light_action).await?;

                match extract_pattern {
                    Some(pattern) => {
                        // qualify extract
                        let should_update_conf = pattern
                            .iter()
                            .map(|(pattern, value_name)| {
                                self.extract_pattern(
                                    (pattern, value_name.as_ref().map(|v| v.as_str())),
                                    &response,
                                    ctx,
                                )
                            })
                            .filter_map(|v| v.ok())
                            .any(|v| v);

                        if should_update_conf {
                            self.db_handler
                                .insert_conf(&Context {
                                    value: serde_json::to_string(&ctx)
                                        .expect("Error serializing context"),
                                })
                                .await?;
                        }
                    }
                    None => {
                        if !no_print {
                            self.print_response(&response)?;
                        }
                    }
                }
            }
            Err(e) => println!("Error: {}", e),
        };

        Ok(())
    }

    pub async fn fetch(
        &self,
        url: &str,
        verb: &str,
        headers: &HashMap<String, String>,
        query_params: &Option<HashMap<String, String>>,
        body: &Option<String>,
    ) -> anyhow::Result<(String, u16, Duration)> {
        let form = headers
            .get("Content-Type")
            .map(|v| v == &"application/x-www-form-urlencoded");

        //
        let mut builder = match verb {
            "POST" => match form {
                Some(true) => self.client.post(url).form(
                    &serde_json::from_str::<HashMap<String, String>>(
                        body.as_ref()
                            .map(|v| v.as_str())
                            .expect("A body was expected"),
                    )
                    .expect("Expected body for POST request"),
                ),
                _ => self
                    .client
                    .post(url)
                    .body(body.clone().expect("Expected body for POST request")),
            },
            "GET" => self.client.get(url),
            _ => panic!("Unsupported verb: {}", verb),
        };

        // query params
        if query_params.is_some() {
            builder = builder.query(query_params.as_ref().unwrap());
        }

        // headers
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            header_map.insert(
                k.parse::<HeaderName>().expect("Unparseable header name"),
                HeaderValue::from_bytes(v.as_bytes()).expect("Unparseable header value"),
            );
        }

        // launching request
        let start = Instant::now();
        let response = builder.headers(header_map).send().await?;
        let duration = start.elapsed();
        println!("Request took: {:?}", duration);

        // getting status and response
        let status = response.status();
        let text: String = response.text().await?;
        Ok((text, status.as_u16().into(), duration))
    }

    pub async fn run_action(
        &self,
        action: &mut Action,
        path_params: &Option<HashMap<String, String>>,
        query_params: &Option<HashMap<String, String>>,
        body: &Option<String>,
        extract_pattern: &Option<HashMap<String, Option<String>>>,
        no_print: bool,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        // get associated project
        let project = self.db_handler.get_project(&action.project_name).await?;

        // extend conf
        let mut conf = project.get_conf();
        conf.extend(ctx.iter().map(|(k, v)| (k.clone(), v.clone())));

        let headers = action.headers_as_map();
        let computed_headers = self.interpolate_hashmap(&headers, &conf);
        debug!("computed headers {:?}", computed_headers);

        let computed_body = self.interpolate_body_from_cli(&body, &conf);

        let computed_full_url = self.interpolate_path_params(&action, &project, path_params, &conf);

        let computed_query_params = query_params
            .as_ref()
            .map(|query_params| self.interpolate_hashmap(&query_params, &conf));

        // interpolate
        let result = self
            .fetch(
                &computed_full_url,
                action.verb.as_str(),
                &computed_headers,
                &computed_query_params,
                &computed_body,
            )
            .await;

        self.handle_result(
            action,
            &computed_body,
            &result,
            extract_pattern,
            no_print,
            ctx,
        )
        .await?;

        // insert history line !
        self.db_handler
            .insert_history(&History {
                id: None,
                action_name: action.name.clone(),
                url: computed_full_url,
                body: computed_body,
                headers: Some(serde_json::to_string(&computed_headers)?),
                response: result
                    .as_ref()
                    .map(|(response, _, _)| response.clone())
                    .ok(),
                status_code: result.as_ref().map(|(_, status, _)| *status).unwrap_or(0),
                duration: result
                    .as_ref()
                    .map(|(_, _, duration)| duration.as_secs_f32())
                    .unwrap_or(0.0),
                timestamp: None,
            })
            .await?;

        Ok(())
    }
}
