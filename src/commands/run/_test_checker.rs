use crate::commands::run::action::R;
use crate::http::FetchResult;
use assert_json_diff::{assert_json_eq, assert_json_include};
use crossterm::style::Stylize;
use indicatif::ProgressBar;
use serde_json::{from_str, Value};
use std::collections::HashMap;
use std::fmt::Display;
use std::panic::catch_unwind;
use std::str::FromStr;

fn get_args<T>(fn_with_args: &str, fn_name: &str) -> Result<T, <T as FromStr>::Err>
where
    T: FromStr,
{
    let to_be_replaced = format!("{}(", fn_name);
    fn_with_args
        .replace(&to_be_replaced, "")
        .replace(')', "")
        .parse::<T>()
}

const STATUS_CODE: &str = "STATUS_CODE";
const JSON_INCLUDE: &str = "JSON_INCLUDE";
const JSON_EQ: &str = "JSON_EQ";
const INT: &str = "INT";
const FLOAT: &str = "FLOAT";
const REGEX: &str = "REGEX";
const EMAIL: &str = "EMAIL";

#[derive(Debug)]
pub enum TestFn {
    StatusCode,
    JsonInclude(String),
    JsonEq(String),
    Int(i64),
    Float(f64),
    NoMatch,
    Regex(regex::Regex),
    Email(regex::Regex),
}

impl FromStr for TestFn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s == STATUS_CODE {
            TestFn::StatusCode
        } else if s.starts_with(JSON_INCLUDE) {
            TestFn::JsonInclude(get_args::<String>(s, JSON_INCLUDE)?)
        } else if s.starts_with(JSON_EQ) {
            TestFn::JsonEq(get_args::<String>(s, JSON_EQ)?)
        } else if s.starts_with(INT) {
            TestFn::Int(get_args::<i64>(s, INT)?)
        } else if s.starts_with(FLOAT) {
            TestFn::Float(get_args::<f64>(s, INT)?)
        } else if s.starts_with(REGEX) {
            TestFn::Regex(regex::Regex::new(s)?)
        } else if s.starts_with(EMAIL) {
            TestFn::Email(regex::Regex::new("^[\\w-\\.]+@([\\w-]+\\.)+[\\w-]{2,4}$")?)
        } else {
            TestFn::NoMatch
        })
    }
}

pub struct TestChecker<'a> {
    pub fetch_results: &'a Vec<R>,
    pub ctx: &'a HashMap<String, String>,
    pub expected: &'a HashMap<String, String>,
}

impl<'a> TestChecker<'a> {
    /// tests are ran after finishing all the requests
    /// So no need to pass the progress bar here to avoid stdout
    /// conflicts
    pub fn print_err(&self, key: &str, got: &str, expected: &str) {
        let gott = if got.is_empty() { "<empty str>" } else { got };
        let r = format!("   Expected '{}' to be `{}` got `{}`", key, expected, gott).red();
        println!("{}", r);
    }

    /// default check for int, float and strings
    fn default_check<T>(&self, key: &str, expected: T, ctx: &HashMap<String, String>) -> bool
    where
        T: FromStr + Display + PartialEq,
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let ctx_value = ctx.get(key).and_then(|v| v.parse::<T>().ok()).unwrap();
        if ctx_value != expected {
            self.print_err(key, &ctx_value.to_string(), &expected.to_string());
            return false;
        }
        true
    }

    /// Special check for regex
    fn regex_based_check(&self, key: &str, regex: &regex::Regex, ctx: &HashMap<String, String>) -> bool {
        match ctx.get(key) {
            Some(ctx_value) => {
                let is_err = !regex.is_match(ctx_value);
                if is_err {
                    self.print_err(key, ctx_value, regex.as_str());
                }
                is_err
            }
            None => {
                self.print_err(key, "<empty str>", regex.as_str());
                false
            }
        }
    }

    pub fn _check(&self, result: &FetchResult, ctx: &HashMap<String, String>) -> bool {
        let r = self
            .expected
            .iter()
            .map(|(key, value)| match TestFn::from_str(key.as_str()) {
                Ok(TestFn::StatusCode) => {
                    let status_code = result.status.to_string();
                    if status_code.as_str() != value {
                        self.print_err(STATUS_CODE, &status_code, value);
                        return false;
                    }
                    true
                }
                Ok(TestFn::NoMatch) => match TestFn::from_str(value) {
                    Ok(TestFn::JsonInclude(json_to_test)) => catch_unwind(|| {
                        assert_json_include!(
                            actual: from_str::<Value>(&result.response).unwrap(),
                            expected: from_str::<Value>(&json_to_test).unwrap()
                        );
                    })
                    .is_ok(),
                    Ok(TestFn::JsonEq(json_to_test)) => catch_unwind(|| {
                        assert_json_eq!(
                            from_str::<Value>(&result.response).unwrap(),
                            from_str::<Value>(&json_to_test).unwrap()
                        );
                    })
                    .is_ok(),
                    Ok(TestFn::Int(expected)) => self.default_check(key, expected, ctx),
                    Ok(TestFn::Float(expected)) => self.default_check(key, expected, ctx),
                    Ok(TestFn::Regex(regex)) => self.regex_based_check(key, &regex, ctx),
                    Ok(TestFn::Email(regex)) => self.regex_based_check(key, &regex, ctx),
                    Ok(TestFn::NoMatch) => match ctx.get(key) {
                        Some(ctx_value) => {
                            let is_err = ctx_value != value;
                            if is_err {
                                self.print_err(key, ctx_value, value);
                            }
                            is_err
                        }
                        None => {
                            self.print_err(key, "<empty str>", value);
                            false
                        }
                    },
                    _ => false,
                },
                _ => false,
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
