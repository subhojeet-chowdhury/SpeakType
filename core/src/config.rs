use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Hotkey string, e.g. "CONTROL+ALT+SPACE". Press-and-hold: press starts
    /// recording, release stops it and kicks off transcription.
    pub hotkey: String,

    /// Example: "http://127.0.0.1:8080"
    pub whisper_server_url: String,

    /// Where temporary recordings are written before being transcribed.
    pub scratch_dir: PathBuf,

    /// Base URL of the local Python cleanup service (LangGraph cleanup + tone routing).
    pub cleanup_service_url: String,

    /// If false, skip the cleanup service entirely and inject the raw Whisper
    /// transcript. Useful for testing Phase 1 in isolation.
    pub enable_cleanup: bool,

    /// Text injection strategy: "stream" (keystrokes) or "batch" (clipboard).
    pub injection_mode: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("SPEAKTYPE"))
            .set_default("hotkey", "ALT+SPACE")?
            .set_default("whisper_server_url", "http://127.0.0.1:8080")?
            .set_default("scratch_dir", "/tmp/speaktype")?
            .set_default("cleanup_service_url", "http://127.0.0.1:8008")?
            .set_default("enable_cleanup", true)?
            .set_default("injection_mode", "batch")?
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
