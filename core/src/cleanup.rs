use anyhow::{Context, Result};
use serde::Serialize;
use crate::inject::Injector;

#[derive(Serialize)]
struct CleanupRequest<'a> {
    raw_transcript: &'a str,
    app_context: &'a str,
}

/// Sends the raw Whisper transcript to the local Python cleanup service
/// and injects the text chunks as they arrive over the HTTP stream.
pub async fn clean_transcript(
    service_url: &str,
    raw_transcript: &str,
    app_context: &str,
    injector: &mut Injector,
) -> Result<()> {
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

    let mut resp = resp.error_for_status().context("cleanup service returned an error")?;
    
    let mut final_text = String::new();
    while let Some(chunk) = resp.chunk().await.context("error reading stream chunk")? {
        if let Ok(text) = std::str::from_utf8(&chunk) {
            final_text.push_str(text);
            if let Err(e) = injector.inject_chunk(text) {
                tracing::error!("failed to inject chunk: {e}");
            }
        }
    }
    
    tracing::info!("injected (streamed): {final_text}");
    Ok(())
}
