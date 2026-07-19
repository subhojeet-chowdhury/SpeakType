use anyhow::{bail, Context, Result};
use std::path::Path;
use reqwest::multipart;
use serde_json::Value;
use crate::config::AppConfig;

/// Sends a 16kHz mono WAV file to the configured STT provider.
pub async fn transcribe(cfg: &AppConfig, wav_path: &Path) -> Result<String> {
    let audio_data = tokio::fs::read(wav_path)
        .await
        .context("failed to read dictation.wav from disk")?;

    let part = multipart::Part::bytes(audio_data)
        .file_name("dictation.wav")
        .mime_str("audio/wav")?;

    if cfg.stt_provider == "groq" {
        let api_key = cfg.groq_api_key.as_ref().context("stt_provider is groq but groq_api_key is not set in config")?;
        
        let form = multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3-turbo")
            .text("response_format", "json");

        let client = reqwest::Client::new();
        let req_future = client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send();

        let resp = tokio::time::timeout(std::time::Duration::from_secs(30), req_future)
            .await
            .context("groq stt connection timed out after 30 seconds")?
            .context("failed to reach groq stt API")?;

        let resp = resp.error_for_status().context("groq stt returned an error status code")?;

        let json: Value = resp.json().await.context("failed to parse JSON from groq")?;
        
        if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
            Ok(text.trim().to_string())
        } else {
            bail!("groq response missing 'text' field: {:?}", json);
        }
    } else {
        // Default to whisper(local)
        let form = multipart::Form::new().part("file", part);

        let client = reqwest::Client::new();
        let req_future = client
            .post(format!("{}/inference", cfg.whisper_server_url))
            .multipart(form)
            .send();

        let resp = tokio::time::timeout(std::time::Duration::from_secs(30), req_future)
            .await
            .context("whisper server connection timed out after 30 seconds")?
            .context("failed to reach whisper server — is it running on port 8080? (see README)")?;

        let resp = resp.error_for_status().context("whisper server returned an error status code")?;

        let json: Value = resp.json().await.context("failed to parse JSON from whisper server")?;
        
        if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
            Ok(text.trim().to_string())
        } else {
            bail!("whisper server response missing 'text' field: {:?}", json);
        }
    }
}
