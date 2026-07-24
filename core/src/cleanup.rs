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
    injection_mode: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let req_future = client
        .post(format!("{service_url}/cleanup"))
        .json(&CleanupRequest {
            raw_transcript,
            app_context,
        })
        .send();

    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), req_future)
        .await
        .context("cleanup service connection timed out after 10 seconds")?
        .context("failed to reach cleanup service — is it running? (see README)")?;

    let mut resp = resp.error_for_status().context("cleanup service returned an error")?;
    
    let mut final_text = String::new();
    loop {
        let chunk_future = resp.chunk();
        let chunk_res = tokio::time::timeout(std::time::Duration::from_secs(15), chunk_future).await;
        
        let chunk = match chunk_res {
            Ok(res) => res.context("error reading stream chunk")?,
            Err(_) => {
                tracing::warn!("cleanup stream chunk timed out, injecting what we have so far");
                break;
            }
        };
        
        match chunk {
            Some(c) => {
                if let Ok(text) = std::str::from_utf8(&c) {
                    final_text.push_str(text);
                    
                    if injection_mode != "batch" {
                        if let Err(e) = injector.inject_chunk(text) {
                            tracing::error!("failed to inject chunk: {e}");
                        }
                    }
                }
            }
            None => break, // stream finished
        }
    }
    
    if injection_mode == "batch" {
        tracing::info!("injected (batch): {final_text}");
        if let Err(e) = injector.inject_batch(&final_text) {
            tracing::error!("failed to inject batch via clipboard: {e}");
        }
    } else {
        tracing::info!("injected (streamed): {final_text}");
    }

    Ok(())
}


git add .
git commit -m "feat: added unified start script" -m "- Added start.sh unified runner to automatically build, configure, and boot the Whisper, FastAPI, and Rust daemon services simultaneously." -m "- Implemented process trap in start.sh for graceful background service shutdown on CTRL+C." -m "- Updated README.md to reflect Windows support and simplified the Quick Start guide." 
