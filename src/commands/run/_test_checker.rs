use crate::commands::run::action::R;
use crate::http::FetchResult;
use assert_json_diff::{assert_json_eq, assert_json_include};
use crossterm::style::Stylize;
use indicatif::ProgressBar;
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

    pub fn print_err(&self, key: &str, got: &str, expected: &str) {
        let gott = if got.is_empty() { "EMPTY STR" } else { got };
        let r = format!("   Expected '{}' to be `{}` got `{}`", key, expected, gott).red();
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
                        self.print_err("STATUS_CODE", &status_code, value);
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
                            self.print_err(key, ctx_value, value);
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

    pub fn check(&self, flow_name: &str, pb: &ProgressBar) -> Vec<bool> {
        let f = format!(
            "{} {}...",
            "🐞 Analyzing results for".green(),
            flow_name.green()
        );
        pb.suspend(|| println!("{}", f));

        let mut r = vec![];
        for fetch_result in self.fetch_results {
            let status_code = fetch_result
                .result
                .as_ref()
                .map(|r| r.status.to_string())
                .unwrap_or("".to_string());

            let result = fetch_result.result.as_ref();
            let is_success = match &result {
                Ok(f) => {
                    let ctx = &fetch_result.ctx;
                    let check_r = self._check(f, ctx);
                    if check_r {
                        pb.suspend(|| {
                            println!(
                                "   {} {}",
                                "🦄 ??Checking...".green(),
                                "Tests passed ✅".green()
                            )
                        });
                    } else {
                        pb.suspend(|| {
                            println!(
                                "   {} {}",
                                "🦄 ??Checking...".red(),
                                " Some tests failed ❌".red()
                            )
                        });
                    }
                    check_r
                }
                Err(_) => {
                    println!("   {} {} ❌", "🦄 ??Checking...".red(), status_code);
                    false
                }
            };
            r.push(is_success);
        }
        r
    }
}
