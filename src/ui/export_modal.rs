use std::path::{Path, PathBuf};
use egui::{vec2, Align, Align2, Color32, Context, Layout, ProgressBar, RichText, Window};
use crate::video::{
    start_export, ActiveExport, AvailableEncoders, CropRectPixels, ExportProgressUpdate,
    ExportSettings, QualityPreset, VideoCodecOption, VideoMetadata,
};
use crate::utils::time::format_time;

#[derive(Clone, Debug, PartialEq)]
pub enum ExportModalState {
    Closed,
    Configure,
    Exporting {
        output_path: PathBuf,
        percent: f32,
        current_time_sec: f64,
        speed: String,
        fps: f32,
    },
    Finished(PathBuf),
    Error(String),
}

pub struct ExportModal {
    pub state: ExportModalState,
    pub output_path: PathBuf,
    pub codec: VideoCodecOption,
    pub quality: QualityPreset,
    pub include_audio: bool,
    pub active_export: Option<ActiveExport>,
    pub encoders: AvailableEncoders,
}

impl Default for ExportModal {
    fn default() -> Self {
        Self {
            state: ExportModalState::Closed,
            output_path: PathBuf::new(),
            codec: VideoCodecOption::H264Auto,
            quality: QualityPreset::High,
            include_audio: true,
            active_export: None,
            encoders: AvailableEncoders::default(),
        }
    }
}

impl ExportModal {
    pub fn new() -> Self {
        Self {
            encoders: AvailableEncoders::detect(),
            ..Default::default()
        }
    }

    pub fn open(&mut self, metadata: &VideoMetadata, default_dir: Option<&Path>) {
        let input_path = &metadata.path;
        let stem = input_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());

        let parent = default_dir
            .or_else(|| input_path.parent())
            .unwrap_or_else(|| Path::new("."));

        self.output_path = parent.join(format!("{}_cropped.mp4", stem));
        self.state = ExportModalState::Configure;
    }

    pub fn close(&mut self) {
        if let Some(ref active) = self.active_export {
            active.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.active_export = None;
        self.state = ExportModalState::Closed;
    }

    pub fn update(&mut self, ctx: &Context) {
        let mut updates = Vec::new();
        if let Some(ref active) = self.active_export {
            while let Ok(update) = active.progress_rx.try_recv() {
                updates.push(update);
            }
        }

        for update in updates {
            match update {
                ExportProgressUpdate::Started => {
                    self.state = ExportModalState::Exporting {
                        output_path: self.output_path.clone(),
                        percent: 0.0,
                        current_time_sec: 0.0,
                        speed: "1.0x".to_string(),
                        fps: 0.0,
                    };
                    ctx.request_repaint();
                }
                ExportProgressUpdate::Progress {
                    percent,
                    current_time_sec,
                    speed,
                    fps,
                } => {
                    self.state = ExportModalState::Exporting {
                        output_path: self.output_path.clone(),
                        percent,
                        current_time_sec,
                        speed,
                        fps,
                    };
                    ctx.request_repaint();
                }
                ExportProgressUpdate::Finished(path) => {
                    self.state = ExportModalState::Finished(path);
                    self.active_export = None;
                    ctx.request_repaint();
                }
                ExportProgressUpdate::Error(err) => {
                    self.state = ExportModalState::Error(err);
                    self.active_export = None;
                    ctx.request_repaint();
                }
                ExportProgressUpdate::Cancelled => {
                    self.state = ExportModalState::Configure;
                    self.active_export = None;
                    ctx.request_repaint();
                }
            }
        }
    }

    pub fn render(
        &mut self,
        ctx: &Context,
        metadata: &VideoMetadata,
        crop_pixels: Option<CropRectPixels>,
        start_time: f64,
        end_time: f64,
    ) {
        if self.state == ExportModalState::Closed {
            return;
        }

        let is_crop_active = crop_pixels.is_some();
        let target_duration = (end_time - start_time).max(0.001);

        let mut is_open = true;
        Window::new("Export Video")
            .open(&mut is_open)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .fixed_size(vec2(520.0, 360.0))
            .show(ctx, |ui| {
                match &self.state {
                    ExportModalState::Configure => {
                        ui.heading("Export Video Settings");
                        ui.add_space(8.0);

                        // File destination row
                        ui.horizontal(|ui| {
                            ui.label("Save To:");
                            let path_str = self.output_path.to_string_lossy().to_string();
                            ui.add(
                                egui::TextEdit::singleline(&mut path_str.clone())
                                    .desired_width(340.0)
                                    .interactive(false),
                            );
                            if ui.button("Browse...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("MP4 Video", &["mp4"])
                                    .set_file_name(&format!(
                                        "{}_cropped.mp4",
                                        metadata
                                            .path
                                            .file_stem()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "output".to_string())
                                    ))
                                    .save_file()
                                {
                                    self.output_path = path;
                                }
                            }
                        });

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Details summary
                        egui::Grid::new("export_summary_grid")
                            .num_columns(2)
                            .spacing([20.0, 8.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Trim Range:").strong());
                                ui.label(format!(
                                    "{} → {} (Duration: {})",
                                    format_time(start_time),
                                    format_time(end_time),
                                    format_time(target_duration)
                                ));
                                ui.end_row();

                                ui.label(RichText::new("Crop Dimensions:").strong());
                                if let Some(crop) = crop_pixels {
                                    ui.label(format!(
                                        "{} × {} px (at x:{}, y:{}) [Source: {} × {}]",
                                        crop.width, crop.height, crop.x, crop.y, metadata.width, metadata.height
                                    ));
                                } else {
                                    ui.label(format!("Full Size ({} × {} px)", metadata.width, metadata.height));
                                }
                                ui.end_row();

                                ui.label(RichText::new("Video Codec:").strong());
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("codec_select")
                                        .selected_text(match self.codec {
                                            VideoCodecOption::H264Auto => "H.264 (MP4 - Recommended)",
                                            VideoCodecOption::H265Auto => "H.265 / HEVC (High Efficiency)",
                                            VideoCodecOption::H264Software => "H.264 (Software libx264)",
                                            VideoCodecOption::H265Software => "H.265 (Software libx265)",
                                            VideoCodecOption::LosslessCopy => "Lossless Stream Copy (Fast Trim)",
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.codec, VideoCodecOption::H264Auto, "H.264 (MP4 - Recommended)");
                                            ui.selectable_value(&mut self.codec, VideoCodecOption::H265Auto, "H.265 / HEVC (High Efficiency)");
                                            ui.selectable_value(&mut self.codec, VideoCodecOption::H264Software, "H.264 (Software libx264)");
                                            ui.selectable_value(&mut self.codec, VideoCodecOption::H265Software, "H.265 (Software libx265)");
                                            if !is_crop_active {
                                                ui.selectable_value(&mut self.codec, VideoCodecOption::LosslessCopy, "Lossless Stream Copy (Fast Trim)");
                                            }
                                        });

                                    // Hardware acceleration badge
                                    if self.codec == VideoCodecOption::H264Auto || self.codec == VideoCodecOption::H265Auto {
                                        if self.encoders.nvenc_h264 || self.encoders.nvenc_hevc {
                                            ui.label(RichText::new("⚡ NVENC Active").size(11.0).color(Color32::from_rgb(120, 220, 120)));
                                        } else if self.encoders.qsv_h264 || self.encoders.qsv_hevc {
                                            ui.label(RichText::new("⚡ Intel QSV Active").size(11.0).color(Color32::from_rgb(120, 200, 255)));
                                        } else if self.encoders.amf_h264 || self.encoders.amf_hevc {
                                            ui.label(RichText::new("⚡ AMD AMF Active").size(11.0).color(Color32::from_rgb(255, 150, 120)));
                                        }
                                    }
                                });
                                ui.end_row();

                                if self.codec != VideoCodecOption::LosslessCopy {
                                    ui.label(RichText::new("Quality:").strong());
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(&mut self.quality, QualityPreset::High, "High Quality");
                                        ui.selectable_value(&mut self.quality, QualityPreset::Medium, "Balanced");
                                        ui.selectable_value(&mut self.quality, QualityPreset::Low, "Smaller Size");
                                    });
                                    ui.end_row();
                                }

                                ui.label(RichText::new("Audio Track:").strong());
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.include_audio, "Include Audio");
                                });
                                ui.end_row();
                            });

                        ui.add_space(20.0);

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let export_btn = ui.add_sized([100.0, 30.0], egui::Button::new(
                                RichText::new("Start Export").strong().color(Color32::BLACK)
                            ).fill(Color32::from_rgb(255, 190, 40)));

                            if export_btn.clicked() {
                                let settings = ExportSettings {
                                    input_path: metadata.path.clone(),
                                    output_path: self.output_path.clone(),
                                    crop_rect: crop_pixels,
                                    start_time,
                                    end_time,
                                    codec: self.codec,
                                    quality: self.quality,
                                    include_audio: self.include_audio,
                                };
                                self.active_export = Some(start_export(settings, &self.encoders));
                            }

                            if ui.button("Cancel").clicked() {
                                self.state = ExportModalState::Closed;
                            }
                        });
                    }

                    ExportModalState::Exporting {
                        output_path,
                        percent,
                        current_time_sec,
                        speed,
                        fps,
                    } => {
                        ui.heading("Exporting Video...");
                        ui.add_space(12.0);

                        ui.add(
                            ProgressBar::new(*percent)
                                .show_percentage()
                                .animate(true)
                        );
                        ui.add_space(10.0);

                        ui.label(format!(
                            "Progress: {} / {} ({})",
                            format_time(*current_time_sec),
                            format_time(target_duration),
                            format!("{:.1}%", percent * 100.0)
                        ));
                        ui.label(format!("Speed: {} | FPS: {:.1}", speed, fps));
                        ui.label(
                            RichText::new(format!("Saving to: {}", output_path.display()))
                                .size(11.0)
                                .color(Color32::GRAY),
                        );

                        ui.add_space(20.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Abort Export").clicked() {
                                if let Some(ref active) = self.active_export {
                                    active.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        });
                    }

                    ExportModalState::Finished(path) => {
                        ui.heading(RichText::new("✓ Export Completed!").color(Color32::from_rgb(100, 220, 100)));
                        ui.add_space(10.0);

                        ui.label(format!("Saved successfully to:\n{}", path.display()));
                        ui.add_space(18.0);

                        ui.horizontal(|ui| {
                            if ui.button("▶ Open Video").clicked() {
                                let _ = open::that(path);
                            }
                            if ui.button("📁 Show in Folder").clicked() {
                                if let Some(parent) = path.parent() {
                                    let _ = open::that(parent);
                                }
                            }
                        });

                        ui.add_space(16.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Done").clicked() {
                                self.state = ExportModalState::Closed;
                            }
                        });
                    }

                    ExportModalState::Error(err) => {
                        ui.heading(RichText::new("⚠ Export Failed").color(Color32::from_rgb(255, 90, 90)));
                        ui.add_space(10.0);
                        ui.label(err.as_str());

                        ui.add_space(20.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Back").clicked() {
                                self.state = ExportModalState::Configure;
                            }
                            if ui.button("Close").clicked() {
                                self.state = ExportModalState::Closed;
                            }
                        });
                    }
                    ExportModalState::Closed => {}
                }
            });

        if !is_open {
            self.close();
        }
    }
}

