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
        Ok(Self { enigo })
    }

    pub fn inject_chunk(&mut self, text: &str) -> Result<()> {
        self.enigo
            .text(text)
            .context("failed to simulate keystrokes for injection")?;
        Ok(())
    }
}

pub fn inject_text(text: &str) -> Result<()> {
    let mut injector = Injector::new()?;
    injector.inject_chunk(text)
}
