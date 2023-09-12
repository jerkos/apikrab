use crate::commands::run::action::R;
use crate::http::FetchResult;
use crossterm::style::Stylize;
use std::collections::HashMap;

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
                    let empty_str = "".to_string();
                    let ctx_value = ctx.get(key).unwrap_or(&empty_str);
                    if ctx_value != value {
                        self.print_err(ctx_value, value);
                        return false;
                    }
                    true
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
        if let Err(_) = result {
            println!("   {} {}", "🦄".red(), "Error while fetching".red());
            return false;
        }
        let unwrapped_result = result.as_ref().unwrap();
        let ctx = &last_result.ctx;

        self._check(unwrapped_result, ctx)
    }
}
