use std::collections::HashMap;
use std::time::Instant;

use colored::Colorize;
use log::debug;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::db::dao::{Context, LightAction};
use crate::db::db_handler::DBHandler;
use crate::{json_path, utils::replace_with_conf};

pub struct Api {
    client: reqwest::Client,
}

impl Api {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
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

    pub fn interpolate_headers_from_cli(
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
        action: &LightAction,
        project: &crate::db::dao::Project,
        path_params: &Option<HashMap<String, String>>,
        ctx: &HashMap<String, String>,
    ) -> String {
        // getting an url like /api/v1/users/{id} and a hashmap like {id: 1}
        // next check in query params if exists then in conf

        let full_url = format!("{}/{}", project.test_url.as_ref().unwrap(), action.url);
        path_params
            .as_ref()
            .map(|path_params| replace_with_conf(&full_url, &path_params))
            .map(|full_url| replace_with_conf(&full_url, ctx))
            .unwrap_or_else(|| replace_with_conf(&full_url, ctx))
    }

    pub async fn handle_result(
        &self,
        light_action: &mut LightAction,
        body: &Option<String>,
        result: anyhow::Result<(String, u16)>,
        db_handler: &DBHandler,
        extract_pattern: &Option<String>,
        no_print: bool,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        match result {
            Ok((response, status)) => {
                let status_code = format!("Status code: {}", status);
                if status >= 400 {
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
                db_handler.upsert_action(&light_action).await?;

                match extract_pattern {
                    Some(pattern) => {
                        // qualify extract
                        let mut extracted = pattern.split(":");
                        let (pattern_to_extract, value_name) =
                            (extracted.next().unwrap(), extracted.next());
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
                                Err(e) => {
                                    println!("Error: {}", e);
                                    "".to_string()
                                }
                            })
                            .unwrap_or("".to_string());
                        println!("Extracted: \n{}", extracted_as_string);
                        if let Some(value_name) = value_name {
                            ctx.insert(value_name.to_string(), extracted_as_string);
                            db_handler
                                .insert_conf(&Context {
                                    value: serde_json::to_string(&ctx)
                                        .expect("Error serializing context"),
                                })
                                .await?;
                        }
                    }
                    None => {
                        if !no_print {
                            println!("Received response: ");
                            let response_as_value =
                                serde_json::from_str::<serde_json::Value>(&response)?;
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&response_as_value)?
                                    .split("\n")
                                    .take(10)
                                    .collect::<Vec<&str>>()
                                    .join("\n")
                            );
                            println!("...");
                        }
                    }
                }
            }
            //}
            Err(e) => println!("Error: {}", e),
        };
        Ok(())
    }

    pub async fn fetch(
        &self,
        url: &str,
        verb: &str,
        headers: &HashMap<String, String>,
        body: &Option<String>,
    ) -> anyhow::Result<(String, u16)> {
        let form = headers
            .get("Content-Type")
            .map(|v| v == &"application/x-www-form-urlencoded");

        let builder = match verb {
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
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            header_map.insert(
                k.parse::<HeaderName>().expect("Unparseable header name"),
                HeaderValue::from_bytes(v.as_bytes()).expect("Unparseable header value"),
            );
        }
        let start = Instant::now();
        let response = builder.headers(header_map).send().await?;
        let duration = start.elapsed();
        println!("Request took: {:?}", duration);
        let status = response.status();
        let text: String = response.text().await?;
        Ok((text, status.as_u16().into()))
    }

    pub async fn run_action(
        &self,
        action: &mut LightAction,
        computed_path_params: &Option<HashMap<String, String>>,
        body: &Option<String>,
        db_handler: &DBHandler,
        extract_pattern: &Option<String>,
        no_print: bool,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        // get associated project
        let project = db_handler.get_project(&action.project_name).await?;

        // extend conf
        let mut conf = project.get_conf();
        conf.extend(ctx.iter().map(|(k, v)| (k.clone(), v.clone())));

        let headers = action.headers_as_map();
        let computed_headers = self.interpolate_headers_from_cli(&headers, &conf);
        debug!("computed headers {:?}", computed_headers);

        let computed_body = self.interpolate_body_from_cli(&body, &conf);

        let computed_full_url =
            self.interpolate_path_params(&action, &project, computed_path_params, &conf);

        // interpolate
        let result = self
            .fetch(
                &computed_full_url,
                action.verb.as_str(),
                &computed_headers,
                &computed_body,
            )
            .await;

        self.handle_result(
            action,
            &computed_body,
            result,
            db_handler,
            extract_pattern,
            no_print,
            ctx,
        )
        .await?;
        Ok(())
    }
}
