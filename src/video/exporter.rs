use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use crossbeam_channel::{unbounded, Receiver, Sender};
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodecOption {
    H264Auto,
    H265Auto,
    H264Software,
    H265Software,
    LosslessCopy, // Only when no crop
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    High,     // CRF 18
    Medium,   // CRF 23
    Low,      // CRF 28
}

#[derive(Debug, Clone)]
pub struct ExportSettings {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub crop_rect: Option<CropRectPixels>,
    pub start_time: f64,
    pub end_time: f64,
    pub codec: VideoCodecOption,
    pub quality: QualityPreset,
    pub include_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRectPixels {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum ExportProgressUpdate {
    Started,
    Progress {
        percent: f32,
        current_time_sec: f64,
        speed: String,
        fps: f32,
    },
    Finished(PathBuf),
    Error(String),
    Cancelled,
}

pub struct ActiveExport {
    pub cancel_flag: Arc<AtomicBool>,
    pub progress_rx: Receiver<ExportProgressUpdate>,
}

#[derive(Debug, Clone, Default)]
pub struct AvailableEncoders {
    pub nvenc_h264: bool,
    pub nvenc_hevc: bool,
    pub qsv_h264: bool,
    pub qsv_hevc: bool,
    pub amf_h264: bool,
    pub amf_hevc: bool,
    pub videotoolbox_h264: bool,
    pub videotoolbox_hevc: bool,
}

impl AvailableEncoders {
    pub fn detect() -> Self {
        let output = crate::utils::process::create_hidden_command("ffmpeg")
            .args(["-encoders", "-v", "quiet"])
            .output();

        let mut enc = AvailableEncoders::default();
        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout);
            enc.nvenc_h264 = s.contains("h264_nvenc");
            enc.nvenc_hevc = s.contains("hevc_nvenc");
            enc.qsv_h264 = s.contains("h264_qsv");
            enc.qsv_hevc = s.contains("hevc_qsv");
            enc.amf_h264 = s.contains("h264_amf");
            enc.amf_hevc = s.contains("hevc_amf");
            enc.videotoolbox_h264 = s.contains("h264_videotoolbox");
            enc.videotoolbox_hevc = s.contains("hevc_videotoolbox");
        }
        enc
    }
}

pub fn start_export(settings: ExportSettings, encoders: &AvailableEncoders) -> ActiveExport {
    let (tx, rx) = unbounded();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel_flag.clone();
    let encoders_clone = encoders.clone();

    thread::spawn(move || {
        run_export_thread(settings, encoders_clone, tx, cancel_clone);
    });

    ActiveExport {
        cancel_flag,
        progress_rx: rx,
    }
}

fn run_export_thread(
    settings: ExportSettings,
    encoders: AvailableEncoders,
    tx: Sender<ExportProgressUpdate>,
    cancel_flag: Arc<AtomicBool>,
) {
    let _ = tx.send(ExportProgressUpdate::Started);

    let total_target_duration = (settings.end_time - settings.start_time).max(0.001);

    let mut cmd = crate::utils::process::create_hidden_command("ffmpeg");
    cmd.arg("-y"); // Overwrite output

    // Time seeking before input for fast seek
    cmd.args(["-ss", &format!("{:.3}", settings.start_time)]);

    // Input file
    cmd.arg("-i").arg(&settings.input_path);

    // Duration limit
    cmd.args(["-t", &format!("{:.3}", total_target_duration)]);

    let has_crop = if let Some(crop) = settings.crop_rect {
        // Enforce even dimensions for YUV420p
        let w = (crop.width & !1).max(2);
        let h = (crop.height & !1).max(2);
        let x = crop.x & !1;
        let y = crop.y & !1;
        cmd.args(["-vf", &format!("crop={}:{}:{}:{}", w, h, x, y)]);
        true
    } else {
        false
    };

    // Determine codec
    let can_stream_copy = !has_crop && settings.codec == VideoCodecOption::LosslessCopy;

    if can_stream_copy {
        cmd.args(["-c:v", "copy"]);
    } else {
        let (codec_name, crf_param, crf_val) = match settings.codec {
            VideoCodecOption::H264Auto => {
                if encoders.videotoolbox_h264 {
                    ("h264_videotoolbox", "-q:v", match settings.quality {
                        QualityPreset::High => "75",
                        QualityPreset::Medium => "60",
                        QualityPreset::Low => "45",
                    })
                } else if encoders.nvenc_h264 {
                    ("h264_nvenc", "-cq", match settings.quality {
                        QualityPreset::High => "19",
                        QualityPreset::Medium => "24",
                        QualityPreset::Low => "28",
                    })
                } else if encoders.qsv_h264 {
                    ("h264_qsv", "-global_quality", match settings.quality {
                        QualityPreset::High => "20",
                        QualityPreset::Medium => "25",
                        QualityPreset::Low => "30",
                    })
                } else if encoders.amf_h264 {
                    ("h264_amf", "-rc", "cqp")
                } else {
                    ("libx264", "-crf", match settings.quality {
                        QualityPreset::High => "18",
                        QualityPreset::Medium => "23",
                        QualityPreset::Low => "28",
                    })
                }
            }
            VideoCodecOption::H265Auto => {
                if encoders.videotoolbox_hevc {
                    ("hevc_videotoolbox", "-q:v", match settings.quality {
                        QualityPreset::High => "75",
                        QualityPreset::Medium => "60",
                        QualityPreset::Low => "45",
                    })
                } else if encoders.nvenc_hevc {
                    ("hevc_nvenc", "-cq", match settings.quality {
                        QualityPreset::High => "21",
                        QualityPreset::Medium => "26",
                        QualityPreset::Low => "31",
                    })
                } else if encoders.qsv_hevc {
                    ("hevc_qsv", "-global_quality", match settings.quality {
                        QualityPreset::High => "22",
                        QualityPreset::Medium => "27",
                        QualityPreset::Low => "32",
                    })
                } else if encoders.amf_hevc {
                    ("hevc_amf", "-rc", "cqp")
                } else {
                    ("libx265", "-crf", match settings.quality {
                        QualityPreset::High => "21",
                        QualityPreset::Medium => "26",
                        QualityPreset::Low => "31",
                    })
                }
            }
            VideoCodecOption::H264Software | VideoCodecOption::LosslessCopy => {
                ("libx264", "-crf", match settings.quality {
                    QualityPreset::High => "18",
                    QualityPreset::Medium => "23",
                    QualityPreset::Low => "28",
                })
            }
            VideoCodecOption::H265Software => {
                ("libx265", "-crf", match settings.quality {
                    QualityPreset::High => "21",
                    QualityPreset::Medium => "26",
                    QualityPreset::Low => "31",
                })
            }
        };

        cmd.args(["-c:v", codec_name]);
        cmd.args([crf_param, crf_val]);
        cmd.args(["-pix_fmt", "yuv420p"]);
        if codec_name.starts_with("libx26") {
            cmd.args(["-preset", "fast"]);
        }
    }

    // Audio settings
    if settings.include_audio {
        if can_stream_copy {
            cmd.args(["-c:a", "copy"]);
        } else {
            cmd.args(["-c:a", "aac", "-b:a", "192k"]);
        }
    } else {
        cmd.arg("-an");
    }

    // Movflags for fast web playback start
    cmd.args(["-movflags", "+faststart"]);

    // Output progress info
    cmd.args(["-progress", "pipe:1"]);
    cmd.arg(&settings.output_path);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(ExportProgressUpdate::Error(format!("Failed to start FFmpeg: {}", e)));
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = tx.send(ExportProgressUpdate::Error("Cannot capture FFmpeg stdout".to_string()));
            return;
        }
    };

    let reader = BufReader::new(stdout);
    let re_out_time_ms = Regex::new(r"out_time_ms=(\d+)").unwrap();
    let re_out_time = Regex::new(r"out_time=(\d{2}:\d{2}:\d{2}\.\d+)").unwrap();
    let re_speed = Regex::new(r"speed=\s*([\d\.]+x)").unwrap();
    let re_fps = Regex::new(r"fps=\s*([\d\.]+)").unwrap();

    let mut current_speed = "1.0x".to_string();
    let mut current_fps = 0.0f32;

    for line in reader.lines() {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = tx.send(ExportProgressUpdate::Cancelled);
            // Clean up incomplete file
            let _ = std::fs::remove_file(&settings.output_path);
            return;
        }

        if let Ok(l) = line {
            if let Some(caps) = re_fps.captures(&l) {
                if let Ok(f) = caps[1].parse::<f32>() {
                    current_fps = f;
                }
            }
            if let Some(caps) = re_speed.captures(&l) {
                current_speed = caps[1].to_string();
            }

            let mut out_sec = None;
            if let Some(caps) = re_out_time_ms.captures(&l) {
                if let Ok(us) = caps[1].parse::<f64>() {
                    out_sec = Some(us / 1_000_000.0);
                }
            } else if let Some(caps) = re_out_time.captures(&l) {
                out_sec = crate::utils::time::parse_time(&caps[1]);
            }

            if let Some(sec) = out_sec {
                let percent = (sec / total_target_duration).clamp(0.0, 1.0) as f32;
                let _ = tx.send(ExportProgressUpdate::Progress {
                    percent,
                    current_time_sec: sec,
                    speed: current_speed.clone(),
                    fps: current_fps,
                });
            }
        }
    }

    match child.wait() {
        Ok(status) => {
            if status.success() {
                let _ = tx.send(ExportProgressUpdate::Progress {
                    percent: 1.0,
                    current_time_sec: total_target_duration,
                    speed: current_speed,
                    fps: current_fps,
                });
                let _ = tx.send(ExportProgressUpdate::Finished(settings.output_path));
            } else if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(ExportProgressUpdate::Cancelled);
            } else {
                let _ = tx.send(ExportProgressUpdate::Error(format!("FFmpeg failed with exit code: {:?}", status.code())));
            }
        }
        Err(e) => {
            let _ = tx.send(ExportProgressUpdate::Error(format!("FFmpeg process error: {}", e)));
        }
    }
}

