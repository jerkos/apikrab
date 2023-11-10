use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::http::FetchResult;

/// Create a new progress bar with custom style
pub fn new_pb(step_count: u64) -> ProgressBar {
    ProgressBar::new(step_count).with_style(
        ProgressStyle::default_bar()
            .template("{spinner} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap(),
    )
}

pub fn init_progress_bars(step_count: u64) -> (MultiProgress, ProgressBar) {
    let multi_bar = MultiProgress::new();
    let main_pb = multi_bar.add(new_pb(step_count));
    (multi_bar, main_pb)
}

/// add a progress bar for the current request
pub fn add_progress_bar_for_request(multi_bar: &MultiProgress, message: &str) -> ProgressBar {
    // creating a progress bar for the current request
    multi_bar.add(
        ProgressBar::new_spinner()
            .with_style(
                ProgressStyle::with_template("{spinner:.blue} {msg}")
                    .unwrap()
                    .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]),
            )
            .with_message(format!("Running {} ", message)),
    )
}

/// finish progress bar
pub fn finish_progress_bar(
    pb: &ProgressBar,
    fetch_result: anyhow::Result<&FetchResult, &anyhow::Error>,
    message: &str,
) {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {msg}")
            .unwrap(),
    );
    match fetch_result.as_ref() {
        Ok(fetch_result) => {
            let status = fetch_result.status.to_string();
            let formatted_str = if fetch_result.is_success() {
                format!("{} ✅", status.green())
            } else {
                format!("{} ❌", status.red())
            };
            pb.finish_with_message(format!("{}  {}", formatted_str, message));
        }
        Err(_) => {
            pb.finish_with_message(format!("{}  {} {}", "❌".red(), message, "io error"));
        }
    }
}
