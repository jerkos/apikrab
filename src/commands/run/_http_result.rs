use crate::commands::run::_printer::Printer;
use crate::http::FetchResult;
use crate::json_path;
use colored::Colorize;
use colored_json::ToColoredJson;
use indicatif::ProgressBar;
use std::collections::HashMap;

/// Print response and extracted values
pub struct HttpResult<'a> {
    pub(crate) fetch_result: anyhow::Result<&'a FetchResult, &'a anyhow::Error>,
    pub(crate) printer: &'a mut Printer,
}

impl<'a> HttpResult<'a> {
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
                    " ⚠️  No value extracted for pattern {}",
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
                    extracted_as_string
                        .to_colored_json_auto()
                        .ok()
                        .unwrap_or_else(|| extracted_as_string.clone()),
                    value_name
                        .map(|v| format!("saved as {}", v.bright_yellow()))
                        .unwrap_or("".to_string())
                )
            });
        });

        Some(extracted_as_string)
    }

    fn print_response(&mut self, response: &str, pb: &ProgressBar) -> anyhow::Result<()> {
        // grep is superior to quiet option
        self.printer.p_response(response, pb);
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
        self.printer.maybe_to_clip(response, pb);
        Ok(())
    }

    pub fn handle_result(
        &mut self,
        extract_pattern: Option<&HashMap<String, Option<String>>>,
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
                        self.printer.maybe_to_clip(&concat_pattern, pb);
                        self.printer.p_response(&concat_pattern, pb);
                    }
                    None => self.print_response(response, pb)?,
                }
            }
            Err(e) => self.printer.p_error(&format!("{}", e), pb),
        };

        Ok(())
    }
}
