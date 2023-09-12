use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Context};
use crate::http::FetchResult;
use crate::json_path;
use colored::Colorize;
use crossterm::style::Stylize;
use std::collections::HashMap;

pub struct HttpResult<'a> {
    pub db_handler: &'a DBHandler,
    pub fetch_result: &'a anyhow::Result<FetchResult>,
}

impl<'a> HttpResult<'a> {
    pub fn new(db_handler: &'a DBHandler, fetch_result: &'a anyhow::Result<FetchResult>) -> Self {
        Self {
            db_handler,
            fetch_result,
        }
    }

    fn extract_pattern(
        &self,
        (pattern_to_extract, value_name): (&str, Option<&str>),
        response: &str,
        ctx: &mut HashMap<String, String>,
        no_print: bool,
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

        if !no_print {
            println!(
                "Extraction of {}: {} {}",
                pattern_to_extract.bright_green(),
                extracted_as_string.bright_magenta(),
                value_name
                    .map(|v| format!("saved as {}", v.bright_yellow()))
                    .unwrap_or("".to_string())
            );
        }

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

    pub async fn handle_result(
        &self,
        action: &mut Action,
        body: &Option<String>,
        extract_pattern: &Option<HashMap<String, Option<String>>>,
        no_print: bool,
        ctx: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        match self.fetch_result {
            Ok(FetchResult {
                response, status, ..
            }) => {
                let status_code = format!("Status code: {}", status);
                if status >= &400 {
                    if !no_print {
                        println!("{}", status_code.bold().red());
                        println!("{}", response);
                    }
                    return Ok(());
                }

                // Successful request
                if !no_print {
                    println!("{}", status_code.bold().green());
                }
                action.response_example = Some(response.clone());
                action.body_example = body.clone();
                self.db_handler.upsert_action(&action).await?;

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
                                    no_print,
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
}
