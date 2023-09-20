use crate::commands::run::action::R;
use crate::http::FetchResult;
use assert_json_diff::{assert_json_eq, assert_json_include};
use crossterm::style::Stylize;
use serde_json::Value;
use std::collections::HashMap;
use std::panic::catch_unwind;

pub struct TestChecker<'a> {
    pub fetch_results: &'a Vec<R>,
    pub ctx: &'a HashMap<String, String>,
    pub expected: &'a HashMap<String, String>,
}

impl<'a> TestChecker<'a> {
    pub fn new(
        fetch_results: &'a Vec<R>,
        ctx: &'a HashMap<String, String>,
        expected: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            fetch_results,
            ctx,
            expected,
        }
    }

    pub fn print_err(&self, got: &str, expected: &str) {
        let r = format!("   Expected {} got {}", expected, got).red();
        println!("{}", r);
    }

    pub fn _check(&self, result: &FetchResult, ctx: &HashMap<String, String>) -> bool {
        let r = self
            .expected
            .iter()
            .map(|(key, value)| match key.as_str() {
                "STATUS_CODE" => {
                    let status_code = result.status.to_string();
                    if status_code.as_str() != value {
                        self.print_err(&status_code, value);
                        return false;
                    }
                    true
                }
                _ => {
                    if value.contains('(') {
                        let mut splitted = value.split('(');
                        let func = splitted.next().unwrap();
                        let args = splitted.next().unwrap().replace(')', "");
                        let response_value = serde_json::from_str::<Value>(&result.response)
                            .expect("Error parsing response as json");
                        let args_value = serde_json::from_str::<Value>(&args)
                            .expect("Error parsing args as json");
                        let r = match func {
                            "JSON_INCLUDE" => catch_unwind(|| {
                                assert_json_include!(actual: response_value, expected: args_value);
                            }),
                            "JSON_EQ" => catch_unwind(|| {
                                assert_json_eq!(response_value, args_value);
                            }),
                            _ => panic!("Unsupported function: {}", func),
                        };
                        r.is_ok()
                    } else {
                        let empty_str = "".to_string();
                        let ctx_value = ctx.get(key).unwrap_or(&empty_str);
                        if ctx_value != value {
                            self.print_err(ctx_value, value);
                            false
                        } else {
                            true
                        }
                    }
                }
            })
            .collect::<Vec<bool>>();

        let all_true = r.iter().all(|b| *b);
        all_true
    }

    pub fn check(&self, flow_name: &str) -> bool {
        let f = format!("{} {}...", " 🐞Running flow".green(), flow_name.green());
        println!("{}", f);

        for fetch_result in self.fetch_results {
            let status_code = fetch_result
                .result
                .as_ref()
                .map(|r| r.status.to_string())
                .unwrap_or("".to_string());

            match &fetch_result.result {
                Ok(_) => println!("   {} {}", "🦄 ??Checking...".green(), status_code),
                Err(_) => println!("   {} {}", "🦄 ??Checking...".red(), status_code),
            }
        }

        let last_result = self.fetch_results.last().unwrap();
        let result = &last_result.result;
        if result.is_err() {
            println!("   {} {}", "🦄".red(), "Error while fetching".red());
            return false;
        }
        let unwrapped_result = result.as_ref().unwrap();
        let ctx = &last_result.ctx;

        self._check(unwrapped_result, ctx)
    }
}
