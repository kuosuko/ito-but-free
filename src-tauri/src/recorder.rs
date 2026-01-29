use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// A Send + Sync handle for an in-progress recording.
///
/// The actual `cpal::Stream` is created and owned inside a dedicated thread
/// because `cpal::Stream` is not `Send`/`Sync` on macOS.
pub struct RecordingSession {
    stop_tx: mpsc::Sender<()>,
    done_rx: mpsc::Receiver<Result<PathBuf>>,
}

impl RecordingSession {
    pub fn start() -> Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<Result<PathBuf>>();

        thread::spawn(move || {
            let res = (|| -> Result<PathBuf> {
                let host = cpal::default_host();
                let device = host
                    .default_input_device()
                    .ok_or_else(|| anyhow!("No default input device"))?;

                let supported_config = device
                    .default_input_config()
                    .context("Failed to get default input config")?;

                let sample_rate = supported_config.sample_rate().0;
                let channels = supported_config.channels();

                let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
                let samples_cb = samples.clone();

                let err_fn = |err| eprintln!("an error occurred on the input audio stream: {err}");

                let stream = match supported_config.sample_format() {
                    cpal::SampleFormat::I16 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[i16], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    buf.extend_from_slice(data);
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::U16 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[u16], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    for &s in data {
                                        let v: i16 = (s as i32 - 32768) as i16;
                                        buf.push(v);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::F32 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[f32], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    for &s in data {
                                        let v: i16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                        buf.push(v);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    other => return Err(anyhow!("Unsupported sample format: {other:?}")),
                };

                stream.play()?;

                // Block until stop signal.
                let _ = stop_rx.recv();
                drop(stream);

                // Write WAV.
                let samples = samples
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock samples"))?;

                let mut path = std::env::temp_dir();
                let filename = format!(
                    "groqtranscriber-{}.wav",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                );
                path.push(filename);

                let spec = hound::WavSpec {
                    channels,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };

                let mut writer =
                    hound::WavWriter::create(&path, spec).context("Failed to create wav")?;
                for &s in samples.iter() {
                    writer.write_sample(s).ok();
                }
                writer.finalize().ok();

                Ok(path)
            })();

            let _ = done_tx.send(res);
        });

        Ok(Self { stop_tx, done_rx })
    }

    pub fn stop_and_save_wav(self) -> Result<PathBuf> {
        let _ = self.stop_tx.send(());
        self.done_rx
            .recv()
            .map_err(|_| anyhow!("Recording thread terminated unexpectedly"))?
    }
}
