use arboard::Clipboard;

pub struct Printer {
    pub no_print: bool,
    pub clipboard: bool,
    pub clipboard_instance: Option<Clipboard>,

    // only full response is send to stdout
    // or extracted values
    pub grepped: bool,
}

impl Printer {
    pub fn new(no_print: bool, clipboard: bool, grepped: bool) -> Self {
        Self {
            no_print,
            clipboard,
            clipboard_instance: if clipboard {
                Some(Clipboard::new().expect("Error initializing clipboard"))
            } else {
                None
            },
            grepped,
        }
    }

    pub fn p_response(&self, print_fn: impl FnOnce()) {
        if self.grepped {
            print_fn();
        }
    }

    pub fn p_info(&self, print_fn: impl FnOnce()) {
        if self.grepped {
            return;
        }
        if !self.no_print {
            print_fn();
        }
    }

    pub fn maybe_to_clip(&mut self, value: &str) {
        if self.clipboard {
            let r = self
                .clipboard_instance
                .as_mut()
                .unwrap()
                .set_text(value.to_owned());
            if r.is_err() {
                println!("Error setting clipboard: {}", r.err().unwrap());
            } else {
                println!("Copied to clipboard !");
            }
        }
    }
}
