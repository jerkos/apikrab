use crate::commands::run::action::R;
use crate::http::FetchResult;
use assert_json_diff::{assert_json_eq, assert_json_include};
use crossterm::style::Stylize;
use indicatif::ProgressBar;
use serde_json::{from_str, Value};
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Display;
use std::panic::catch_unwind;
use std::str::FromStr;

use super::_printer::Printer;

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
const STATUS: &str = "STATUS";
const JSON_INCLUDE: &str = "JSON_INCLUDE";
const JSON_EQ: &str = "JSON_EQ";
const INT: &str = "INT";
const FLOAT: &str = "FLOAT";
const REGEX: &str = "REGEX";
const EMAIL: &str = "EMAIL";

#[derive(Debug)]
pub struct UnaryTestResult {
    pub is_success: bool,
    pub message: String,
    pub expected: Option<String>,
    pub got: Option<String>,
}

impl UnaryTestResult {
    pub fn success(message: String) -> Self {
        Self {
            is_success: true,
            message,
            expected: None,
            got: None,
        }
    }

    pub fn fail(message: String, expected: &str, got: &str) -> Self {
        Self {
            is_success: false,
            message,
            expected: Some(expected.to_string()),
            got: Some(got.to_string()),
        }
    }

    pub fn fail_with_no_info(message: String) -> Self {
        Self {
            is_success: false,
            message,
            expected: None,
            got: None,
        }
    }
}

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
        Ok(if s == STATUS_CODE || s == STATUS {
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
            TestFn::Email(regex::Regex::new(
                r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})",
            )?)
        } else {
            TestFn::NoMatch
        })
    }
}

pub struct TestChecker<'a> {
    pub fetch_results: &'a Vec<R>,
    pub ctx: &'a HashMap<String, String>,
    pub expected: &'a HashMap<String, String>,
    pub printer: &'a Printer,
}

impl<'a> TestChecker<'a> {
    pub fn print_err(&self, key: &str, got: &str, expected: &str, pb: &ProgressBar) {
        let gott = if got.is_empty() { "<empty str>" } else { got };
        let r = format!("   Expected '{}' to be `{}` got `{}`", key, expected, gott).red();
        self.printer.p_info(|| pb.suspend(|| println!("{}", r)));
    }

    fn json_check(
        &self,
        message: String,
        json_test_type: &str,
        test_json_result: Result<(), Box<dyn Any + Send>>,
        result: &FetchResult,
        json_to_test: String,
        pb: &ProgressBar,
    ) -> UnaryTestResult {
        match test_json_result {
            Ok(_) => UnaryTestResult::success(message),
            Err(err) => {
                self.print_err(json_test_type, &result.response, &json_to_test, pb);
                UnaryTestResult::fail(
                    message,
                    err.downcast_ref::<String>()
                        .unwrap_or(&"<failure>".to_string()),
                    &result.response,
                )
            }
        }
    }

    /// default check for int, float and strings
    fn default_check<T>(
        &self,
        message: String,
        key: &str,
        expected: &T,
        ctx: &HashMap<String, String>,
        pb: &ProgressBar,
    ) -> UnaryTestResult
    where
        T: FromStr + Display + PartialEq,
        <T as FromStr>::Err: std::fmt::Debug,
    {
        match ctx.get(key) {
            Some(ctx_value) => {
                let ctx_value = ctx_value.parse::<T>().unwrap();
                if ctx_value != *expected {
                    self.print_err(key, &ctx_value.to_string(), &expected.to_string(), pb);
                    return UnaryTestResult::fail(
                        message,
                        &expected.to_string(),
                        &ctx_value.to_string(),
                    );
                }
                UnaryTestResult::success(message)
            }
            None => {
                self.print_err(key, "<empty str>", &expected.to_string(), pb);
                UnaryTestResult::fail(message, &expected.to_string(), "<empty str>")
            }
        }
    }

    /// Special check for regex
    fn regex_based_check(
        &self,
        message: String,
        key: &str,
        regex: &regex::Regex,
        ctx: &HashMap<String, String>,
        pb: &ProgressBar,
    ) -> UnaryTestResult {
        match ctx.get(key) {
            Some(ctx_value) => {
                let is_err = !regex.is_match(ctx_value);
                if is_err {
                    self.print_err(key, ctx_value, regex.as_str(), pb);
                }
                UnaryTestResult {
                    is_success: !is_err,
                    message,
                    expected: Some(format!("match {}", regex.as_str())),
                    got: Some(format!("unmatch {}", ctx_value)),
                }
            }
            None => {
                self.print_err(key, "<empty str>", regex.as_str(), pb);
                UnaryTestResult::fail(message, regex.as_str(), "<empty str>")
            }
        }
    }

    pub fn _check(
        &self,
        result: &FetchResult,
        ctx: &HashMap<String, String>,
        pb: &ProgressBar,
    ) -> Vec<UnaryTestResult> {
        self.expected
            .iter()
            .map(|(key, value)| {
                let message = format!("{}: {}", key, value);

                match TestFn::from_str(key.as_str()) {
                    // status code
                    Ok(TestFn::StatusCode) => {
                        let status_code = result.status.to_string();
                        if status_code.as_str() != value {
                            self.print_err(STATUS_CODE, &status_code, value, pb);
                            return UnaryTestResult::fail(message, value, &status_code);
                        }
                        UnaryTestResult::success(message)
                    }
                    Ok(TestFn::NoMatch) => match TestFn::from_str(value) {
                        Ok(TestFn::JsonInclude(json_to_test)) => {
                            let test_include_result = catch_unwind(|| {
                                assert_json_include!(
                                    actual: from_str::<Value>(&result.response).unwrap(),
                                    expected: from_str::<Value>(&json_to_test).unwrap()
                                );
                            });
                            self.json_check(
                                message,
                                JSON_INCLUDE,
                                test_include_result,
                                result,
                                json_to_test,
                                pb,
                            )
                        }
                        Ok(TestFn::JsonEq(json_to_test)) => {
                            let test_eq_result = catch_unwind(|| {
                                assert_json_eq!(
                                    from_str::<Value>(&result.response).unwrap(),
                                    from_str::<Value>(&json_to_test).unwrap()
                                );
                            });
                            self.json_check(
                                message,
                                JSON_EQ,
                                test_eq_result,
                                result,
                                json_to_test,
                                pb,
                            )
                        }
                        Ok(TestFn::Int(expected)) => {
                            self.default_check(message, key, &expected, ctx, pb)
                        }
                        Ok(TestFn::Float(expected)) => {
                            self.default_check(message, key, &expected, ctx, pb)
                        }
                        Ok(TestFn::Regex(regex)) => {
                            self.regex_based_check(message, key, &regex, ctx, pb)
                        }
                        Ok(TestFn::Email(regex)) => {
                            self.regex_based_check(message, key, &regex, ctx, pb)
                        }
                        Ok(TestFn::NoMatch) => self.default_check(message, key, value, ctx, pb),
                        _ => UnaryTestResult::fail_with_no_info(message),
                    },
                    // impossible case
                    _ => UnaryTestResult::fail_with_no_info(message),
                }
            })
            .collect::<Vec<UnaryTestResult>>()
    }

    pub fn check(&self, flow_name: &str, pb: &ProgressBar) -> Vec<Vec<UnaryTestResult>> {
        let f = format!(
            "{} {}...",
            "🐞 Analyzing results for".green(),
            flow_name.green()
        );
        self.printer.p_info(|| pb.suspend(|| println!("{}", f)));

        let mut r = vec![];
        for fetch_result in self.fetch_results {
            let status_code = fetch_result
                .result
                .as_ref()
                .map(|r| r.status.to_string())
                .unwrap_or("".to_string());

            let result = fetch_result.result.as_ref();
            let unary_tests_result = match &result {
                Ok(f) => {
                    let ctx = &fetch_result.ctx;
                    let unary_test_results = self._check(f, ctx, pb);
                    let all_tests_passed = unary_test_results.iter().all(|r| r.is_success);
                    if all_tests_passed {
                        self.printer.p_info(|| {
                            pb.suspend(|| {
                                println!(
                                    "   {} {}",
                                    "🦄 ??Checking...".green(),
                                    "Tests passed ✅".green()
                                )
                            });
                        });
                    } else {
                        self.printer.p_info(|| {
                            pb.suspend(|| {
                                println!(
                                    "   {} {}",
                                    "🦄 ??Checking...".red(),
                                    " Some tests failed ❌".red()
                                )
                            });
                        });
                    }
                    unary_test_results
                }
                Err(_) => {
                    self.printer.p_info(|| {
                        pb.suspend(|| {
                            println!("   {} {} ❌", "🦄 ??Checking...".red(), status_code);
                        });
                    });
                    vec![]
                }
            };
            r.push(unary_tests_result);
        }
        r
    }
}
