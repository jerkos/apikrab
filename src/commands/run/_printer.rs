use arboard::Clipboard;
use colored::Colorize;

/// Handle printing configuration and clipboard option
pub struct Printer {
    pub quiet: bool,
    pub clipboard: bool,
    pub clipboard_instance: Option<Clipboard>,

    // only full response is send to stdout
    // or extracted values
    pub grepped: bool,
}

impl Printer {
    pub fn new(quiet: bool, clipboard: bool, grepped: bool) -> Self {
        Self {
            quiet,
            clipboard,
            clipboard_instance: if clipboard {
                Clipboard::new().ok()
            } else {
                None
            },
            grepped,
        }
    }

    pub fn p_response(&self, response: &str, pb: &indicatif::ProgressBar) {
        if self.grepped {
            pb.suspend(|| println!("{}", response));
        }
    }

    pub fn p_info(&self, print_fn: impl FnOnce()) {
        if self.grepped {
            return;
        }
        if !self.quiet {
            print_fn();
        }
    }

    /// Print error in red given a progress bar
    pub fn p_error(&self, printed_str: &str, pb: &indicatif::ProgressBar) {
        if self.grepped {
            return;
        }
        if !self.quiet {
            let f = format!("Error: {}", printed_str).red();
            pb.suspend(|| println!("{}", f));
        }
    }

    pub fn maybe_to_clip(&mut self, value: &str) {
        match self
            .clipboard_instance
            .as_mut()
            .and_then(|c| c.set_text(value.to_owned()).ok())
        {
            Some(_) => println!("Copied to clipboard !"),
            None => println!("Error setting clipboard"),
        }
    }
}
