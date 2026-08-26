use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::ColorImage;

#[derive(Clone, Debug)]
pub struct DecodedFrame {
    pub timestamp: f64,
    pub width: usize,
    pub height: usize,
    pub image: ColorImage,
}

pub enum DecoderCommand {
    LoadFrame { timestamp: f64 },
    ClearCache,
    ChangeFile { path: PathBuf },
}

pub struct VideoDecoder {
    cmd_tx: Sender<DecoderCommand>,
    frame_rx: Receiver<DecodedFrame>,
    cached_frames: HashMap<u64, ColorImage>,
    current_path: Option<PathBuf>,
    pub latest_frame: Option<DecodedFrame>,
    pub is_loading: bool,
    active_flag: Arc<AtomicBool>,
}

impl VideoDecoder {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded::<DecoderCommand>();
        let (frame_tx, frame_rx) = unbounded::<DecodedFrame>();
        let active_flag = Arc::new(AtomicBool::new(true));

        let flag_clone = active_flag.clone();
        thread::spawn(move || {
            let mut current_video_path: Option<PathBuf> = None;
            let mut cache: HashMap<u64, ColorImage> = HashMap::new();

            while flag_clone.load(Ordering::Relaxed) {
                match cmd_rx.recv() {
                    Ok(DecoderCommand::ChangeFile { path }) => {
                        current_video_path = Some(path);
                        cache.clear();
                    }
                    Ok(DecoderCommand::ClearCache) => {
                        cache.clear();
                    }
                    Ok(DecoderCommand::LoadFrame { timestamp }) => {
                        // Drain newer pending requests to skip intermediate frames if scrubbing fast
                        let mut target_time = timestamp;
                        while let Ok(newer_cmd) = cmd_rx.try_recv() {
                            match newer_cmd {
                                DecoderCommand::LoadFrame { timestamp: next_time } => {
                                    target_time = next_time;
                                }
                                DecoderCommand::ChangeFile { path } => {
                                    current_video_path = Some(path);
                                    cache.clear();
                                }
                                DecoderCommand::ClearCache => {
                                    cache.clear();
                                }
                            }
                        }

                        let time_key = (target_time * 10.0).round() as u64; // ~0.1s quantization for cache
                        if let Some(cached_img) = cache.get(&time_key) {
                            let _ = frame_tx.send(DecodedFrame {
                                timestamp: target_time,
                                width: cached_img.width(),
                                height: cached_img.height(),
                                image: cached_img.clone(),
                            });
                            continue;
                        }

                        if let Some(ref path) = current_video_path {
                            if let Some(img) = extract_frame_ffmpeg(path, target_time) {
                                if cache.len() > 60 {
                                    cache.clear(); // simple cache limit
                                }
                                cache.insert(time_key, img.clone());
                                let _ = frame_tx.send(DecodedFrame {
                                    timestamp: target_time,
                                    width: img.width(),
                                    height: img.height(),
                                    image: img,
                                });
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            cmd_tx,
            frame_rx,
            cached_frames: HashMap::new(),
            current_path: None,
            latest_frame: None,
            is_loading: false,
            active_flag,
        }
    }

    pub fn set_file(&mut self, path: &Path) {
        self.current_path = Some(path.to_path_buf());
        self.latest_frame = None;
        self.cached_frames.clear();
        let _ = self.cmd_tx.send(DecoderCommand::ChangeFile {
            path: path.to_path_buf(),
        });
    }

    pub fn request_frame(&mut self, timestamp: f64) {
        self.is_loading = true;
        let _ = self.cmd_tx.send(DecoderCommand::LoadFrame { timestamp });
    }

    pub fn poll_updates(&mut self) -> bool {
        let mut updated = false;
        while let Ok(frame) = self.frame_rx.try_recv() {
            self.latest_frame = Some(frame);
            self.is_loading = false;
            updated = true;
        }
        updated
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        self.active_flag.store(false, Ordering::Relaxed);
    }
}

fn extract_frame_ffmpeg(path: &Path, timestamp: f64) -> Option<ColorImage> {
    let ts_str = format!("{:.3}", timestamp.max(0.0));

    // Fast seek with -ss before -i for keyframe seeking, or combined for precision
    let mut child = crate::utils::process::create_hidden_command("ffmpeg")
        .args([
            "-ss", &ts_str,
            "-i",
        ])
        .arg(path)
        .args([
            "-vframes", "1",
            "-f", "image2pipe",
            "-vcodec", "mjpeg",
            "-q:v", "3",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let mut buffer = Vec::new();
    stdout.read_to_end(&mut buffer).ok()?;
    let _ = child.wait();

    if buffer.is_empty() {
        return None;
    }

    let img = image::load_from_memory(&buffer).ok()?.to_rgba8();
    let width = img.width() as usize;
    let height = img.height() as usize;
    let pixels = img.into_raw();

    Some(ColorImage::from_rgba_unmultiplied(
        [width, height],
        &pixels,
    ))
}

