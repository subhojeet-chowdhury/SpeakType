use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct CleanupRequest<'a> {
    raw_transcript: &'a str,
    app_context: &'a str,
}

#[derive(Deserialize)]
struct CleanupResponse {
    cleaned_text: String,
}

/// Sends the raw Whisper transcript to the local Python cleanup service
/// (small LangGraph: one cleanup node, one tone-routing node keyed on
/// `app_context`) and returns the cleaned, formatted text ready for injection.
pub async fn clean_transcript(
    service_url: &str,
    raw_transcript: &str,
    app_context: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{service_url}/cleanup"))
        .json(&CleanupRequest {
            raw_transcript,
            app_context,
        })
        .send()
        .await
        .context("failed to reach cleanup service — is it running? (see README)")?;

    let resp = resp.error_for_status().context("cleanup service returned an error")?;
    let body: CleanupResponse = resp.json().await.context("failed to parse cleanup response")?;
    Ok(body.cleaned_text)
}
