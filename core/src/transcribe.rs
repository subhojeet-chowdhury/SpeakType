use anyhow::{bail, Context, Result};
use std::path::Path;
use reqwest::multipart;
use serde_json::Value;

/// Sends a 16kHz mono WAV file to the local whisper.cpp HTTP server.
/// This keeps the model loaded in RAM, achieving near-zero latency.
pub async fn transcribe(server_url: &str, wav_path: &Path) -> Result<String> {
    let audio_data = tokio::fs::read(wav_path)
        .await
        .context("failed to read dictation.wav from disk")?;

    let part = multipart::Part::bytes(audio_data)
        .file_name("dictation.wav")
        .mime_str("audio/wav")?;

    let form = multipart::Form::new().part("file", part);

    let client = reqwest::Client::new();
    let req_future = client
        .post(format!("{server_url}/inference"))
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
