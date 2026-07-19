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

use tracing_subscriber::fmt::writer::MakeWriterExt;

// os will use services to delete the log files, sorted by dates
// add int. in config file to put stdout result to void dev/null

fn main() -> Result<()> {
    // Set up daily rolling log files in the `logs/` directory
    let file_appender = tracing_appender::rolling::daily("logs", "speaktype.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Write logs to both stdout (for the terminal) and the rolling file (for the daemon)
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout.and(non_blocking))
        .init();

    let cfg = AppConfig::load()?;
    std::fs::create_dir_all(&cfg.scratch_dir)?;
    
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    
    // --- Startup Validation ---
    tracing::info!("Validating whisper server at {}...", cfg.whisper_server_url);
    if let Err(e) = rt.block_on(async { reqwest::get(&cfg.whisper_server_url).await }) {
        tracing::error!("Could not connect to whisper server. Is it running? ({e})\nSee README or run scripts/setup_whisper.sh");
        anyhow::bail!("whisper server unreachable");
    }

    let health_url = cfg.cleanup_service_url.replace("/cleanup", "/health");
    tracing::info!("Validating cleanup service at {}...", health_url);
    if let Err(e) = rt.block_on(async { reqwest::get(&health_url).await }) {
        tracing::error!("Could not connect to cleanup service. Is the Python uvicorn server running? ({e})");
        anyhow::bail!("cleanup service unreachable");
    }
    // --------------------------

    tracing::info!("speaktype starting. hotkey = {}", cfg.hotkey);
    tracing::info!("hold the hotkey to record, release to transcribe + inject");

    // tao's event loop is what pumps native OS events; global-hotkey needs one
    // running on the main thread to receive press/release notifications.
    let event_loop = EventLoopBuilder::new().build();
    let hotkey_manager = GlobalHotKeyManager::new()?;
    let hotkey = parse_hotkey(&cfg.hotkey)?;
    hotkey_manager.register(hotkey)?;

    let receiver = GlobalHotKeyEvent::receiver();

    // Recorder lives across the press->release span of a single dictation.
    let mut active_recorder: Option<audio::Recorder> = None;
    let is_busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(evt) = receiver.try_recv() {
            match evt.state {
                HotKeyState::Pressed => {
                    if is_busy.load(std::sync::atomic::Ordering::SeqCst) {
                        tracing::warn!("pipeline is busy, ignoring hotkey press");
                        return;
                    }
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
                    let busy_flag = is_busy.clone();
                    
                    busy_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    std::thread::spawn(move || {
                        // Create a single-threaded local runtime for this dictation.
                        // This avoids the `Send` trait bound errors caused by Enigo's macOS implementation,
                        // while still running in the background so we don't freeze the OS event loop!
                        let local_rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap();
                            
                        local_rt.block_on(async move {
                            run_pipeline(cfg_clone, wav_path).await;
                            busy_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                        });
                    });
                }
            }
        }
    });
}

/// The full post-recording pipeline: local Whisper transcription -> optional
/// LLM cleanup (tone-routed by focused app) -> keystroke injection.
async fn run_pipeline(cfg: AppConfig, wav_path: PathBuf) {
    // Privacy guard: automatically delete the scratch audio and text files from disk
    // when this function returns, even if it panics or fails early.
    struct PrivacyGuard(PathBuf);
    impl Drop for PrivacyGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("wav.txt"));
        }
    }
    let _guard = PrivacyGuard(wav_path.clone());

    let pipeline_start = std::time::Instant::now();
    
    let transcribe_start = std::time::Instant::now();

    let raw = match transcribe::transcribe(&cfg.whisper_server_url, &wav_path).await {
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
    let transcribe_time = transcribe_start.elapsed();
    tracing::info!("raw transcript: {raw} (took {}ms)", transcribe_time.as_millis());

    let mut injector = match inject::Injector::new() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("failed to initialize keystroke injector: {e}");
            return;
        }
    };

    if cfg.enable_cleanup {
        let focus_start = std::time::Instant::now();
        let app_context = focus::active_app_name().unwrap_or_else(|_| "unknown".to_string());
        let focus_time = focus_start.elapsed();
        tracing::info!("active app context detected: {} (took {}ms)", app_context, focus_time.as_millis());
        tracing::info!("active app context : {}", app_context);
        
        let cleanup_start = std::time::Instant::now();
        if let Err(e) = cleanup::clean_transcript(&cfg.cleanup_service_url, &raw, &app_context, &mut injector, &cfg.injection_mode).await {
            tracing::warn!("cleanup service failed ({e}), falling back to raw transcript injection");
            
            if cfg.injection_mode == "batch" {
                if let Err(e2) = injector.inject_batch(&raw) {
                    tracing::error!("fallback batch injection failed: {e2}");
                }
            } else {
                if let Err(e2) = injector.inject_chunk(&raw) {
                    tracing::error!("fallback stream injection failed: {e2}");
                }
            }
        }
        
        let cleanup_time = cleanup_start.elapsed();
        
        tracing::info!(
            "\n--- LATENCY SUMMARY ---\n\
            Transcribe : {}ms\n\
            Focus Det. : {}ms\n\
            LLM Cleanup: {}ms\n\
            Total      : {}ms\n\
            -----------------------",
            transcribe_time.as_millis(),
            focus_time.as_millis(),
            cleanup_time.as_millis(),
            pipeline_start.elapsed().as_millis()
        );
    } else {
        if cfg.injection_mode == "batch" {
            if let Err(e) = injector.inject_batch(&raw) {
                tracing::error!("batch injection failed: {e}");
            }
        } else {
            if let Err(e) = injector.inject_chunk(&raw) {
                tracing::error!("stream injection failed: {e}");
            }
        }
        tracing::info!(
            "\n--- LATENCY SUMMARY ---\n\
            Transcribe : {}ms\n\
            Total      : {}ms\n\
            -----------------------",
            transcribe_time.as_millis(),
            pipeline_start.elapsed().as_millis()
        );
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
