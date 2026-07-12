use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;

/// Whisper.cpp expects 16kHz mono PCM. We resample naively (pick every Nth
/// source frame) rather than pulling in a full resampling crate — good
/// enough for speech, and keeps the dependency footprint small since this is
/// a systems project first, not an audio-DSP project.
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// A running recording. cpal's `Stream` is not `Send`, so the stream itself
/// lives entirely inside a dedicated OS thread; this handle only holds a
/// channel to tell that thread to stop, plus the join handle to wait for the
/// WAV file to be finalized before we hand it to Whisper.
pub struct Recorder {
    stop_tx: mpsc::Sender<()>,
    thread: JoinHandle<Result<()>>,
}

impl Recorder {
    /// Spawns a background thread that opens the default input device and
    /// streams audio into `out_path` (16kHz mono WAV) until `stop()` is called.
    pub fn start(out_path: &Path) -> Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        let path: PathBuf = out_path.to_path_buf();

        let thread = std::thread::spawn(move || -> Result<()> {
            let result = record_loop(&path, stop_rx, &ready_tx);
            if let Err(e) = &result {
                tracing::error!("recording thread exited with error: {e}");
            }
            result
        });

        // Block briefly until the stream is confirmed open, so callers can't
        // stop() before recording actually started.
        match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Recorder { stop_tx, thread }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("timed out waiting for audio stream to start")),
        }
    }

    /// Signals the recording thread to stop, waits for the WAV file to be
    /// finalized (flushed + valid header), and returns.
    pub fn stop(self) -> Result<()> {
        let _ = self.stop_tx.send(());
        self.thread
            .join()
            .map_err(|_| anyhow!("recording thread panicked"))??;
        Ok(())
    }
}

fn record_loop(
    out_path: &Path,
    stop_rx: mpsc::Receiver<()>,
    ready_tx: &mpsc::Sender<Result<()>>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = match host
        .default_input_device()
        .ok_or_else(|| anyhow!("no input (microphone) device found"))
    {
        Ok(d) => d,
        Err(e) => {
            let _ = ready_tx.send(Err(anyhow!("{e}")));
            return Err(e);
        }
    };
    let input_config = device.default_input_config()?;
    let source_rate = input_config.sample_rate().0;
    let channels = input_config.channels();
    let sample_format = input_config.sample_format();

    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(out_path, spec)
        .with_context(|| format!("failed to create wav file at {out_path:?}"))?;

    let ratio = source_rate as f32 / TARGET_SAMPLE_RATE as f32;
    let mut frame_acc: f32 = 0.0;

    // Samples are handed from the audio callback (which must be fast and
    // non-blocking) to this thread via a channel; this thread does the file I/O.
    let (sample_tx, sample_rx) = mpsc::channel::<i16>();
    let err_fn = |e| tracing::error!("audio stream error: {e}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &input_config.into(),
            move |data: &[f32], _| {
                downsample_and_send(data, channels, ratio, &mut frame_acc, &sample_tx);
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &input_config.into(),
            move |data: &[i16], _| {
                let floats: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                downsample_and_send(&floats, channels, ratio, &mut frame_acc, &sample_tx);
            },
            err_fn,
            None,
        )?,
        other => {
            let e = anyhow!("unsupported sample format: {other:?}");
            let _ = ready_tx.send(Err(anyhow!("{e}")));
            return Err(e);
        }
    };

    stream.play()?;
    let _ = ready_tx.send(Ok(()));

    // Drain samples into the WAV file until stop() is signalled.
    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }
        match sample_rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(sample) => writer.write_sample(sample)?,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    // Flush any remaining buffered samples.
    while let Ok(sample) = sample_rx.try_recv() {
        writer.write_sample(sample)?;
    }

    drop(stream); // stops audio capture
    writer.finalize()?; // writes correct WAV header/length
    Ok(())
}

fn downsample_and_send(
    data: &[f32],
    channels: u16,
    ratio: f32,
    frame_acc: &mut f32,
    tx: &mpsc::Sender<i16>,
) {
    for frame in data.chunks(channels as usize) {
        *frame_acc += 1.0;
        if *frame_acc >= ratio {
            *frame_acc -= ratio;
            let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
            let sample_i16 = (mono.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            let _ = tx.send(sample_i16);
        }
    }
}
