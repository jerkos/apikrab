use crate::commands::run::_printer::Printer;
use crate::http::FetchResult;
use crate::json_path;
use colored::Colorize;
use crossterm::style::Stylize;
use indicatif::ProgressBar;
use std::collections::HashMap;

/// Print response and extracted values
pub struct HttpResult<'a> {
    pub(crate) fetch_result: &'a anyhow::Result<FetchResult>,
    pub(crate) printer: &'a mut Printer,
}

impl<'a> HttpResult<'a> {
    pub fn new(fetch_result: &'a anyhow::Result<FetchResult>, printer: &'a mut Printer) -> Self {
        Self {
            fetch_result,
            printer,
        }
    }

    fn extract_pattern(
        &mut self,
        (pattern_to_extract, value_name): (&str, Option<&str>),
        response: &str,
        pb: &ProgressBar,
    ) -> Option<String> {
        //anyhow::Result<String> {
        let extracted = json_path::json_path(response, pattern_to_extract);

        let extracted_as_string = extracted
            .map(|value| match serde_json::to_string_pretty(&value) {
                Ok(v) => {
                    if v.starts_with('\"') {
                        v[1..v.len() - 1].to_owned()
                    } else {
                        v
                    }
                }
                Err(_) => "".to_owned(),
            })
            .unwrap_or("".to_owned());

        if extracted_as_string.is_empty() {
            pb.suspend(|| {
                println!(
                    " ⚠️No value extracted for pattern {}",
                    pattern_to_extract.bright_green()
                )
            });
            return None;
        }

        self.printer.p_info(|| {
            pb.suspend(|| {
                println!(
                    "Extraction of {}: {} {}",
                    pattern_to_extract.bright_green(),
                    extracted_as_string.bright_magenta(),
                    value_name
                        .map(|v| format!("saved as {}", v.bright_yellow()))
                        .unwrap_or("".to_string())
                )
            });
        });

        Some(extracted_as_string)
    }

    fn print_response(&mut self, response: &str, pb: &ProgressBar) -> anyhow::Result<()> {
        // grep is superior to no_print option
        self.printer
            .p_response(|| pb.suspend(|| println!("{}", response)));
        // print response as info if needed
        self.printer.p_info(|| {
            pb.suspend(|| println!("Received response: "));
            let response_as_value = serde_json::from_str::<serde_json::Value>(response)
                .unwrap_or(serde_json::Value::Null);
            pb.suspend(|| {
                println!(
                    "{}\n...",
                    serde_json::to_string_pretty(&response_as_value)
                        .unwrap_or("".to_string())
                        .split('\n')
                        .take(10)
                        .collect::<Vec<&str>>()
                        .join("\n")
                        .red()
                )
            });
        });

        // save response to clipboard if necessary
        self.printer.maybe_to_clip(response);
        Ok(())
    }

    pub fn handle_result(
        &mut self,
        extract_pattern: Option<&HashMap<&str, Option<&str>>>,
        ctx: &mut HashMap<String, String>,
        pb: &ProgressBar,
    ) -> anyhow::Result<()> {
        match self.fetch_result {
            Ok(FetchResult { response, .. }) => {
                match extract_pattern {
                    Some(pattern) => {
                        // qualify extract
                        let concat_pattern = pattern
                            .iter()
                            .filter_map(|(pattern, value_name)| {
                                let extracted_pattern =
                                    self.extract_pattern((pattern, None), response, pb);
                                if let (Some(value_name), Some(extracted_pattern)) =
                                    (value_name, extracted_pattern.as_ref())
                                {
                                    ctx.insert(value_name.to_string(), extracted_pattern.clone());
                                }

                                extracted_pattern
                            })
                            .collect::<Vec<String>>()
                            .join("\n");

                        // to clip if necessary and print response if grepped
                        self.printer.maybe_to_clip(&concat_pattern);
                        self.printer
                            .p_response(|| pb.suspend(|| println!("{}", concat_pattern)));
                    }
                    None => self.print_response(response, pb)?,
                }
            }
            Err(e) => println!("Error: {}", e),
        };

        Ok(())
    }
}
