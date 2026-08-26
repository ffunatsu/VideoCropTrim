use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub path: PathBuf,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub file_size_bytes: u64,
    pub rotation: i32,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    duration: Option<String>,
    tags: Option<serde_json::Value>,
    side_data_list: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    size: Option<String>,
}

impl VideoMetadata {
    pub fn probe(path: &Path) -> Result<Self, String> {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let file_size_bytes = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Try ffprobe first
        let output = crate::utils::process::create_hidden_command("ffprobe")
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(info) = serde_json::from_slice::<FfprobeOutput>(&out.stdout) {
                    return Self::from_ffprobe_output(path.to_path_buf(), file_name, file_size_bytes, info);
                }
            }
        }

        // Fallback: parse ffmpeg -i output
        Self::probe_with_ffmpeg(path, file_name, file_size_bytes)
    }

    fn from_ffprobe_output(
        path: PathBuf,
        file_name: String,
        file_size_bytes: u64,
        info: FfprobeOutput,
    ) -> Result<Self, String> {
        let streams = info.streams.unwrap_or_default();
        let video_stream = streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("video"))
            .ok_or_else(|| "No video stream found in file".to_string())?;

        let width = video_stream.width.unwrap_or(0);
        let height = video_stream.height.unwrap_or(0);
        let video_codec = video_stream.codec_name.clone().unwrap_or_else(|| "unknown".to_string());

        let audio_codec = streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("audio"))
            .and_then(|s| s.codec_name.clone());

        // FPS calculation
        let fps = video_stream
            .avg_frame_rate
            .as_deref()
            .or(video_stream.r_frame_rate.as_deref())
            .and_then(parse_fraction)
            .unwrap_or(30.0);

        // Duration calculation
        let duration = info
            .format
            .as_ref()
            .and_then(|f| f.duration.as_deref())
            .and_then(|d| d.parse::<f64>().ok())
            .or_else(|| {
                video_stream
                    .duration
                    .as_deref()
                    .and_then(|d| d.parse::<f64>().ok())
            })
            .unwrap_or(0.0);

        // Rotation check
        let mut rotation = 0;
        if let Some(tags) = &video_stream.tags {
            if let Some(rot_val) = tags.get("rotate") {
                if let Some(r_str) = rot_val.as_str() {
                    rotation = r_str.parse().unwrap_or(0);
                } else if let Some(r_num) = rot_val.as_i64() {
                    rotation = r_num as i32;
                }
            }
        }
        if let Some(side_data) = &video_stream.side_data_list {
            for item in side_data {
                if let Some(rot_val) = item.get("rotation") {
                    if let Some(r_num) = rot_val.as_i64() {
                        rotation = r_num as i32;
                    }
                }
            }
        }

        // Adjust dimensions if rotated 90 or 270 degrees
        let (adj_width, adj_height) = if rotation.abs() == 90 || rotation.abs() == 270 {
            (height, width)
        } else {
            (width, height)
        };

        Ok(VideoMetadata {
            path,
            file_name,
            width: adj_width,
            height: adj_height,
            duration,
            fps,
            video_codec,
            audio_codec,
            file_size_bytes,
            rotation,
        })
    }

    fn probe_with_ffmpeg(
        path: &Path,
        file_name: String,
        file_size_bytes: u64,
    ) -> Result<Self, String> {
        let output = crate::utils::process::create_hidden_command("ffmpeg")
            .arg("-i")
            .arg(path)
            .output()
            .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse Duration: 00:00:00.00
        let mut duration = 0.0;
        let mut width = 1920;
        let mut height = 1080;
        let mut fps = 30.0;
        let mut video_codec = "h264".to_string();
        let mut audio_codec = None;

        for line in stderr.lines() {
            if line.contains("Duration:") {
                if let Some(dur_str) = line.split("Duration:").nth(1).and_then(|s| s.split(',').next()) {
                    if let Some(secs) = crate::utils::time::parse_time(dur_str) {
                        duration = secs;
                    }
                }
            }
            if line.contains("Video:") {
                let parts: Vec<&str> = line.split("Video:").collect();
                if parts.len() > 1 {
                    let v_info = parts[1];
                    let v_parts: Vec<&str> = v_info.split(',').collect();
                    if let Some(first) = v_parts.first() {
                        video_codec = first.trim().split(' ').next().unwrap_or("h264").to_string();
                    }
                    for part in &v_parts {
                        let p = part.trim();
                        if p.contains('x') && !p.contains("fps") && !p.contains("kb/s") {
                            let dims: Vec<&str> = p.split('x').collect();
                            if dims.len() == 2 {
                                if let (Ok(w), Ok(h)) = (dims[0].trim().parse::<u32>(), dims[1].split(' ').next().unwrap_or("0").parse::<u32>()) {
                                    if w > 0 && h > 0 {
                                        width = w;
                                        height = h;
                                    }
                                }
                            }
                        }
                        if p.ends_with("fps") || p.ends_with("tbr") {
                            if let Some(fps_str) = p.split_whitespace().next() {
                                if let Ok(f) = fps_str.parse::<f64>() {
                                    fps = f;
                                }
                            }
                        }
                    }
                }
            }
            if line.contains("Audio:") {
                audio_codec = Some("aac".to_string());
            }
        }

        Ok(VideoMetadata {
            path: path.to_path_buf(),
            file_name,
            width,
            height,
            duration,
            fps,
            video_codec,
            audio_codec,
            file_size_bytes,
            rotation: 0,
        })
    }
}

fn parse_fraction(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].parse().ok()?;
        let den: f64 = parts[1].parse().ok()?;
        if den != 0.0 {
            return Some(num / den);
        }
    } else if let Ok(v) = s.parse::<f64>() {
        return Some(v);
    }
    None
}

