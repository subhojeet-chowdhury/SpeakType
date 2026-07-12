use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Hotkey string, e.g. "CONTROL+ALT+SPACE". Press-and-hold: press starts
    /// recording, release stops it and kicks off transcription.
    pub hotkey: String,

    /// Path to a compiled whisper.cpp `main`/`whisper-cli` binary.
    pub whisper_bin: PathBuf,

    /// Path to a ggml whisper model, e.g. ggml-base.en.bin.
    pub whisper_model: PathBuf,

    /// Where temporary recordings are written before being transcribed.
    pub scratch_dir: PathBuf,

    /// Base URL of the local Python cleanup service (LangGraph cleanup + tone routing).
    pub cleanup_service_url: String,

    /// If false, skip the cleanup service entirely and inject the raw Whisper
    /// transcript. Useful for testing Phase 1 in isolation.
    pub enable_cleanup: bool,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("SPEAKTYPE"))
            .set_default("hotkey", "ALT+SPACE")?
            .set_default("whisper_bin", "./whisper.cpp/build/bin/whisper-cli")?
            .set_default("whisper_model", "./whisper.cpp/models/ggml-base.en.bin")?
            .set_default("scratch_dir", "/tmp/speaktype")?
            .set_default("cleanup_service_url", "http://127.0.0.1:8008")?
            .set_default("enable_cleanup", true)?
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
