use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::process::Command;

/// Runs local whisper.cpp inference on a 16kHz mono WAV file and returns the
/// raw transcript. This is the "local, offline, private" leg of the pipeline —
/// no audio ever leaves the machine.
///
/// We shell out to the compiled `whisper-cli` binary rather than binding via
/// FFI (whisper-rs). Shelling out is slightly slower to start per-call, but
/// it keeps this crate's build simple (no bundled C++ compile step) and
/// makes it trivial to swap the STT engine later (e.g. faster-whisper)
/// without touching Rust code — you only change the config paths.
pub async fn transcribe(whisper_bin: &Path, model: &Path, wav_path: &Path) -> Result<String> {
    if !whisper_bin.exists() {
        bail!(
            "whisper binary not found at {:?} — build whisper.cpp first (see README)",
            whisper_bin
        );
    }
    if !model.exists() {
        bail!(
            "whisper model not found at {:?} — download a ggml model first (see README)",
            model
        );
    }

    // -nt: no timestamps, -otxt: write a .txt file next to the wav,
    // -l en: assume English (make configurable later for multilingual use).
    let output = Command::new(whisper_bin)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav_path)
        .arg("-nt")
        .arg("-otxt")
        .arg("-l")
        .arg("en")
        .output()
        .await
        .context("failed to spawn whisper.cpp process")?;

    if !output.status.success() {
        bail!(
            "whisper.cpp exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let txt_path = wav_path.with_extension("wav.txt");
    let transcript = tokio::fs::read_to_string(&txt_path)
        .await
        .with_context(|| format!("expected whisper output at {txt_path:?}"))?;

    Ok(transcript.trim().to_string())
}
