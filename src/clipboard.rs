pub struct Clipboard {
    clipboard: Option<arboard::Clipboard>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self {
            clipboard: init_arboard(),
        }
    }
}

impl Clipboard {
    pub fn get(&mut self) -> Option<String> {
        if let Some(clipboard) = &mut self.clipboard {
            match clipboard.get_text() {
                Ok(text) => Some(text),
                Err(err) => {
                    eprintln!("Paste error: {}", err);
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn set(&mut self, text: String) {
        if let Some(clipboard) = &mut self.clipboard {
            if let Err(err) = clipboard.set_text(text) {
                eprintln!("Copy/Cut error: {}", err);
            }
        }
    }
}

fn init_arboard() -> Option<arboard::Clipboard> {
    match arboard::Clipboard::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            eprintln!("Failed to initialize clipboard: {}", err);
            None
        }
    }
}

