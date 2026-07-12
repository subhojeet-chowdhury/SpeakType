mod audio;
mod cleanup;
mod config;
mod focus;
mod inject;
mod transcribe;

use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::path::PathBuf;
use std::str::FromStr;
use tao::event_loop::{ControlFlow, EventLoopBuilder};

use config::AppConfig;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = AppConfig::load()?;
    std::fs::create_dir_all(&cfg.scratch_dir)?;

    tracing::info!("speaktype starting. hotkey = {}", cfg.hotkey);
    tracing::info!("hold the hotkey to record, release to transcribe + inject");

    // tao's event loop is what pumps native OS events; global-hotkey needs one
    // running on the main thread to receive press/release notifications.
    let event_loop = EventLoopBuilder::new().build();
    let hotkey_manager = GlobalHotKeyManager::new()?;
    let hotkey = parse_hotkey(&cfg.hotkey)?;
    hotkey_manager.register(hotkey)?;

    let receiver = GlobalHotKeyEvent::receiver();
    let rt = tokio::runtime::Runtime::new()?;

    // Recorder lives across the press->release span of a single dictation.
    let mut active_recorder: Option<audio::Recorder> = None;

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(evt) = receiver.try_recv() {
            match evt.state {
                HotKeyState::Pressed => {
                    if active_recorder.is_some() {
                        return; // already recording, ignore repeat press
                    }
                    let wav_path = cfg.scratch_dir.join("dictation.wav");
                    tracing::info!("recording started");
                    match audio::Recorder::start(&wav_path) {
                        Ok(rec) => active_recorder = Some(rec),
                        Err(e) => tracing::error!("failed to start recording: {e}"),
                    }
                }
                HotKeyState::Released => {
                    let Some(rec) = active_recorder.take() else { return };
                    tracing::info!("recording stopped, transcribing...");
                    if let Err(e) = rec.stop() {
                        tracing::error!("failed to finalize recording: {e}");
                        return;
                    }
                    let wav_path = cfg.scratch_dir.join("dictation.wav");
                    let cfg_clone = cfg.clone();
                    rt.block_on(run_pipeline(cfg_clone, wav_path));
                }
            }
        }
    });
}

/// The full post-recording pipeline: local Whisper transcription -> optional
/// LLM cleanup (tone-routed by focused app) -> keystroke injection.
async fn run_pipeline(cfg: AppConfig, wav_path: PathBuf) {
    let raw = match transcribe::transcribe(&cfg.whisper_bin, &cfg.whisper_model, &wav_path).await {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            tracing::warn!("transcript was empty, nothing to inject");
            return;
        }
        Err(e) => {
            tracing::error!("transcription failed: {e}");
            return;
        }
    };
    tracing::info!("raw transcript: {raw}");

    let mut injector = match inject::Injector::new() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("failed to initialize keystroke injector: {e}");
            return;
        }
    };

    if cfg.enable_cleanup {
        let app_context = focus::active_app_name().unwrap_or_else(|_| "unknown".to_string());
        if let Err(e) = cleanup::clean_transcript(&cfg.cleanup_service_url, &raw, &app_context, &mut injector).await {
            tracing::warn!("cleanup service failed ({e}), falling back to raw transcript injection");
            if let Err(e2) = injector.inject_chunk(&raw) {
                tracing::error!("fallback injection failed: {e2}");
            }
        }
    } else {
        if let Err(e) = injector.inject_chunk(&raw) {
            tracing::error!("injection failed: {e}");
        }
    }
}

fn parse_hotkey(spec: &str) -> Result<HotKey> {
    // Accepts strings like "CONTROL+ALT+SPACE".
    let parts: Vec<&str> = spec.split('+').map(|s| s.trim()).collect();
    let (key_part, mod_parts) = parts.split_last().expect("hotkey string must not be empty");

    let mut mods = Modifiers::empty();
    for m in mod_parts {
        mods |= match m.to_uppercase().as_str() {
            "CONTROL" | "CTRL" => Modifiers::CONTROL,
            "ALT" => Modifiers::ALT,
            "SHIFT" => Modifiers::SHIFT,
            "SUPER" | "META" | "CMD" => Modifiers::SUPER,
            other => anyhow::bail!("unknown modifier in hotkey config: {other}"),
        };
    }
    let key_upper = key_part.to_uppercase();
    let code = match key_upper.as_str() {
        "SPACE" => Code::Space,
        "ENTER" => Code::Enter,
        "TAB" => Code::Tab,
        "ESCAPE" | "ESC" => Code::Escape,
        _ => Code::from_str(&key_upper)
            .or_else(|_| Code::from_str(key_part))
            .or_else(|_| Code::from_str(&format!("Key{}", key_upper))) // handles 'A' -> 'KeyA'
            .or_else(|_| Code::from_str(&format!("Digit{}", key_upper))) // handles '1' -> 'Digit1'
            .map_err(|_| anyhow::anyhow!("unrecognized key in hotkey config: {key_part}"))?
    };

    Ok(HotKey::new(Some(mods), code))
}
