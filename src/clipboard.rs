/// Handles interfacing either with the OS clipboard.
/// If the "clipboard" feature is off it will instead simulate the clipboard locally.

#[cfg(feature = "clipboard")]
pub struct Clipboard {
    clipboard: Option<copypasta::ClipboardContext>,
}

#[cfg(not(feature = "clipboard"))]
pub struct Clipboard {
    /// Fallback manual clipboard.
    clipboard: String,
}

impl Default for Clipboard {
    #[cfg(feature = "clipboard")]
    fn default() -> Self {
        Self {
            clipboard: init_copypasta(),
        }
    }

    #[cfg(not(feature = "clipboard"))]
    fn default() -> Self {
        Self {
            clipboard: String::default(),
        }
    }
}

impl Clipboard {
    #[cfg(feature = "clipboard")]
    pub fn get(&mut self) -> Option<String> {
        if let Some(clipboard) = &mut self.clipboard {
            use copypasta::ClipboardProvider as _;
            match clipboard.get_contents() {
                Ok(contents) => Some(contents),
                Err(err) => {
                    eprintln!("Paste error: {}", err);
                    None
                }
            }
        } else {
            None
        }
    }

    #[cfg(not(feature = "clipboard"))]
    pub fn get(&mut self) -> Option<String> {
        Some(self.clipboard.clone())
    }

    #[cfg(feature = "clipboard")]
    pub fn set(&mut self, text: String) {
        if let Some(clipboard) = &mut self.clipboard {
            use copypasta::ClipboardProvider as _;
            if let Err(err) = clipboard.set_contents(text) {
                eprintln!("Copy/Cut error: {}", err);
            }
        }
    }

    #[cfg(not(feature = "clipboard"))]
    pub fn set(&mut self, text: String) {
        self.clipboard = text;
    }
}

#[cfg(feature = "clipboard")]
fn init_copypasta() -> Option<copypasta::ClipboardContext> {
    match copypasta::ClipboardContext::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            eprintln!("Failed to initialize clipboard: {}", err);
            None
        }
    }
}
