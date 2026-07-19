use anyhow::{Context, Result};
use enigo::{Enigo, Keyboard, Settings};

/// Injects `text` into the currently-focused text field by simulating
/// keystrokes at the OS input level. This is deliberately dumb about *what*
/// application is focused — it doesn't need to know, because keystroke
/// injection is application-agnostic. That's what makes this approach work
/// "everywhere" rather than needing per-app integrations.
///
/// Trade-off worth knowing: typing character-by-character is slow for long
/// transcripts and can trip autocomplete/autocorrect in some apps. The
/// alternative — writing to the OS clipboard and simulating Ctrl+V — is
/// faster and more reliable, but clobbers whatever the user had copied.
/// We default to direct typing here; see README for the clipboard-paste
/// variant and when to prefer it.
pub struct Injector {
    enigo: Enigo,
}

impl Injector {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default()).context("failed to init enigo")?;
        // let settings = Settings {
        //     mac_delay: 20000, // 20ms delay prevents macOS autocorrect from scrambling text
        //     ..Default::default()
        // };
        // let enigo = Enigo::new(&settings).context("failed to init enigo")?;
        Ok(Self { enigo })
    }

    pub fn inject_chunk(&mut self, text: &str) -> Result<()> {
        self.enigo
            .text(text)
            .context("failed to simulate keystrokes for injection")?;
        Ok(())
    }

    pub fn inject_batch(&mut self, text: &str) -> Result<()> {
        use std::thread;
        use std::time::Duration;
        
        let mut clipboard = arboard::Clipboard::new().context("failed to access clipboard")?;
        
        // 1. Save old clipboard (ignore if it's not text or missing)
        let old_text = clipboard.get_text().unwrap_or_default();
        
        // 2. Set new text
        clipboard.set_text(text).context("failed to set clipboard text")?;
        
        // Give the OS a tiny fraction of a second to register the new clipboard contents
        thread::sleep(Duration::from_millis(50));
        
        // 3. Simulate Cmd+V (Mac) or Ctrl+V (Windows/Linux)
        #[cfg(target_os = "macos")]
        {
            use enigo::{Key, Direction};
            self.enigo.key(Key::Meta, Direction::Press).unwrap();
            self.enigo.key(Key::Unicode('v'), Direction::Click).unwrap();
            self.enigo.key(Key::Meta, Direction::Release).unwrap();
        }
        #[cfg(not(target_os = "macos"))]
        {
            use enigo::{Key, Direction};
            self.enigo.key(Key::Control, Direction::Press).unwrap();
            self.enigo.key(Key::Unicode('v'), Direction::Click).unwrap();
            self.enigo.key(Key::Control, Direction::Release).unwrap();
        }

        // Give the OS time to process the paste before we yank the text back out
        thread::sleep(Duration::from_millis(150));
        
        // 4. Restore old clipboard
        if !old_text.is_empty() {
            let _ = clipboard.set_text(old_text);
        }

        Ok(())
    }
}
