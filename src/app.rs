use std::path::Path;
use std::time::Instant;
use egui::{
    vec2, Align, Align2, Color32, Context, CornerRadius, Key, Layout, Pos2, Rect, RichText, Stroke,
    TextureHandle, TextureOptions, Ui,
};

use crate::ui::{
    render_crop_overlay, render_timeline, render_transport_controls, setup_custom_theme,
    AspectRatioPreset, CropState, ExportModal, TrimState,
};
use crate::video::{VideoDecoder, VideoMetadata};

pub struct VideoCropTrimApp {
    pub metadata: Option<VideoMetadata>,
    pub decoder: VideoDecoder,
    pub crop_state: CropState,
    pub trim_state: TrimState,
    pub export_modal: ExportModal,
    pub video_texture: Option<TextureHandle>,
    pub is_playing: bool,
    pub playback_anchor: Option<(Instant, f64)>,
    pub crop_mode_enabled: bool,
    pub show_metadata_details: bool,
    pub last_requested_time: f64,
    pub ffmpeg_available: bool,
    pub error_message: Option<String>,
}

impl Default for VideoCropTrimApp {
    fn default() -> Self {
        let ffmpeg_available = crate::utils::process::is_ffmpeg_installed();
        Self {
            metadata: None,
            decoder: VideoDecoder::new(),
            crop_state: CropState::default(),
            trim_state: TrimState::default(),
            export_modal: ExportModal::new(),
            video_texture: None,
            is_playing: false,
            playback_anchor: None,
            crop_mode_enabled: true,
            show_metadata_details: false,
            last_requested_time: -1.0,
            ffmpeg_available,
            error_message: None,
        }
    }
}

impl VideoCropTrimApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_theme(&cc.egui_ctx);
        Self::default()
    }

    pub fn recheck_ffmpeg(&mut self) {
        crate::utils::process::ensure_path_env();
        self.ffmpeg_available = crate::utils::process::is_ffmpeg_installed();
        if self.ffmpeg_available {
            self.error_message = None;
            self.export_modal.encoders = crate::video::AvailableEncoders::detect();
        }
    }

    pub fn load_video(&mut self, path: &Path) {
        if !self.ffmpeg_available {
            self.recheck_ffmpeg();
            if !self.ffmpeg_available {
                self.error_message = Some(
                    "FFmpeg is not installed or not found in PATH.".to_string(),
                );
                return;
            }
        }

        self.error_message = None;
        match VideoMetadata::probe(path) {
            Ok(meta) => {
                let duration = meta.duration;
                let w = meta.width;
                let h = meta.height;
                self.trim_state = TrimState::new(duration);
                self.crop_state = CropState::default();
                self.decoder.set_file(path);
                self.decoder.request_frame(0.0);
                self.last_requested_time = 0.0;
                self.video_texture = None;
                self.is_playing = false;
                self.playback_anchor = None;
                self.crop_state.set_preset(AspectRatioPreset::Free, w, h);
                self.metadata = Some(meta);
            }
            Err(e) => {
                eprintln!("Failed to probe video: {}", e);
                self.error_message = Some(format!("Failed to load video: {}", e));
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        if self.metadata.is_none() || self.export_modal.state != crate::ui::ExportModalState::Closed {
            return;
        }

        let fps = self.metadata.as_ref().map(|m| m.fps).unwrap_or(30.0);

        ctx.input(|i| {
            // Space: Play / Pause
            if i.key_pressed(Key::Space) {
                self.toggle_play();
            }

            // Left / Right step
            let shift = i.modifiers.shift;
            if i.key_pressed(Key::ArrowLeft) {
                self.is_playing = false;
                if shift {
                    self.trim_state.step_seconds(-1.0);
                } else {
                    self.trim_state.step_frames(-1, fps);
                }
                self.request_frame_at_current();
            }
            if i.key_pressed(Key::ArrowRight) {
                self.is_playing = false;
                if shift {
                    self.trim_state.step_seconds(1.0);
                } else {
                    self.trim_state.step_frames(1, fps);
                }
                self.request_frame_at_current();
            }

            // I / O for In / Out points
            if i.key_pressed(Key::I) {
                self.trim_state.set_in_to_current();
            }
            if i.key_pressed(Key::O) {
                self.trim_state.set_out_to_current();
            }

            // C: Toggle Crop overlay
            if i.key_pressed(Key::C) {
                self.crop_mode_enabled = !self.crop_mode_enabled;
            }

            // Ctrl + O: Open file
            if i.modifiers.ctrl && i.key_pressed(Key::O) {
                self.open_file_dialog();
            }

            // Ctrl + S or Ctrl + E: Export
            if i.modifiers.ctrl && (i.key_pressed(Key::S) || i.key_pressed(Key::E)) {
                self.open_export_modal();
            }
        });
    }

    fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
        if self.is_playing {
            self.playback_anchor = Some((Instant::now(), self.trim_state.current_time));
        } else {
            self.playback_anchor = None;
        }
    }

    fn request_frame_at_current(&mut self) {
        let t = self.trim_state.current_time;
        if (t - self.last_requested_time).abs() > 0.03 {
            self.decoder.request_frame(t);
            self.last_requested_time = t;
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mov", "mkv", "webm", "avi", "m4v", "flv"])
            .pick_file()
        {
            self.load_video(&path);
        }
    }

    fn open_export_modal(&mut self) {
        if let Some(ref meta) = self.metadata {
            self.export_modal.open(meta, None);
        }
    }
}

impl eframe::App for VideoCropTrimApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle dropped files from OS
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(path) = i.raw.dropped_files.first().and_then(|f| f.path.clone()) {
                    self.load_video(&path);
                }
            }
        });

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Update active export progress
        self.export_modal.update(ctx);

        // Playback timing update
        if self.is_playing {
            if let Some((anchor_inst, anchor_time)) = self.playback_anchor {
                let elapsed = anchor_inst.elapsed().as_secs_f64();
                let mut next_time = anchor_time + elapsed;

                // Loop playback inside trim range
                if next_time > self.trim_state.end_time {
                    next_time = self.trim_state.start_time;
                    self.playback_anchor = Some((Instant::now(), next_time));
                }

                self.trim_state.seek_to(next_time);
                self.request_frame_at_current();
                ctx.request_repaint();
            }
        }

        // Poll decoded frames from worker thread
        if self.decoder.poll_updates() {
            if let Some(ref frame) = self.decoder.latest_frame {
                self.video_texture = Some(ctx.load_texture(
                    "current_video_frame",
                    frame.image.clone(),
                    TextureOptions::LINEAR,
                ));
            }
        }

        // TOP BAR
        let mut dismiss_error = false;
        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);

                if ui.button("📂 Open Video").on_hover_text("Open video file (Ctrl+O)").clicked() {
                    self.open_file_dialog();
                }

                if !self.ffmpeg_available {
                    ui.separator();
                    ui.label(
                        RichText::new("⚠️ FFmpeg is not installed")
                            .color(Color32::from_rgb(255, 120, 100))
                            .strong(),
                    );
                    if ui.button("🔄 Recheck").on_hover_text("Recheck PATH and FFmpeg installation status").clicked() {
                        self.recheck_ffmpeg();
                    }
                }

                if let Some(ref meta) = self.metadata {
                    ui.separator();
                    ui.label(
                        RichText::new(&meta.file_name)
                            .strong()
                            .color(Color32::WHITE),
                    );

                    ui.label(
                        RichText::new(format!("({}×{}, {:.1} fps)", meta.width, meta.height, meta.fps))
                            .size(12.0)
                            .color(Color32::GRAY),
                    );

                    ui.separator();

                    // Crop toggle
                    let crop_btn = ui.selectable_label(self.crop_mode_enabled, "✂ Spatial Crop");
                    if crop_btn.on_hover_text("Toggle Crop Mode (C)").clicked() {
                        self.crop_mode_enabled = !self.crop_mode_enabled;
                    }

                    if self.crop_mode_enabled {
                        // Aspect ratio dropdown
                        egui::ComboBox::from_id_salt("aspect_ratio_select")
                            .selected_text(self.crop_state.aspect_preset.label())
                            .show_ui(ui, |ui| {
                                let presets = [
                                    AspectRatioPreset::Free,
                                    AspectRatioPreset::Original,
                                    AspectRatioPreset::Ratio1x1,
                                    AspectRatioPreset::Ratio16x9,
                                    AspectRatioPreset::Ratio9x16,
                                    AspectRatioPreset::Ratio4x3,
                                    AspectRatioPreset::Ratio3x2,
                                    AspectRatioPreset::Ratio21x9,
                                    AspectRatioPreset::Ratio4x5,
                                ];
                                for p in presets {
                                    if ui.selectable_label(self.crop_state.aspect_preset == p, p.label()).clicked() {
                                        self.crop_state.set_preset(p, meta.width, meta.height);
                                    }
                                }
                            });

                        if self.crop_state.is_cropped() {
                            if ui.button("↺ Reset Crop").clicked() {
                                self.crop_state.reset();
                            }
                        }
                    }

                    // Export button on the right
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let export_btn = ui.add_sized(
                            [110.0, 26.0],
                            egui::Button::new(
                                RichText::new("⚡ Export Video")
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(Color32::from_rgb(255, 185, 30)),
                        );

                        if export_btn.on_hover_text("Export Cropped & Trimmed Video (Ctrl+E)").clicked() {
                            self.open_export_modal();
                        }
                    });
                }
            });

            // Error banner if any
            if let Some(ref err) = self.error_message {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠️ {}", err))
                            .color(Color32::from_rgb(255, 100, 100))
                            .size(13.0),
                    );
                    if ui.small_button("✕").clicked() {
                        dismiss_error = true;
                    }
                });
            }

            ui.add_space(4.0);
        });

        if dismiss_error {
            self.error_message = None;
        }

        // BOTTOM BAR (TIMELINE & TRANSPORT)
        if self.metadata.is_some() {
            egui::TopBottomPanel::bottom("bottom_timeline").show(ctx, |ui| {
                ui.add_space(6.0);

                let fps = self.metadata.as_ref().map(|m| m.fps).unwrap_or(30.0);

                // Transport controls row
                let mut is_playing = self.is_playing;
                if render_transport_controls(ui, &mut self.trim_state, &mut is_playing, fps) {
                    self.is_playing = false;
                    self.playback_anchor = None;
                    self.request_frame_at_current();
                }
                if is_playing != self.is_playing {
                    self.toggle_play();
                }

                ui.add_space(4.0);

                // Range timeline slider
                if render_timeline(ui, &mut self.trim_state, fps) {
                    if self.is_playing {
                        self.playback_anchor = Some((Instant::now(), self.trim_state.current_time));
                    }
                    self.request_frame_at_current();
                }

                ui.add_space(6.0);
            });
        }

        // CENTRAL VIDEO PREVIEW CANVAS
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref meta) = self.metadata {
                let avail_size = ui.available_size();
                if avail_size.x <= 10.0 || avail_size.y <= 10.0 {
                    return;
                }

                // Compute aspect-fit video rect
                let video_aspect = meta.width as f32 / meta.height as f32;
                let avail_aspect = avail_size.x / avail_size.y;

                let (video_display_w, video_display_h) = if video_aspect > avail_aspect {
                    (avail_size.x, avail_size.x / video_aspect)
                } else {
                    (avail_size.y * video_aspect, avail_size.y)
                };

                let center = ui.min_rect().min + avail_size / 2.0;
                let video_screen_rect = Rect::from_center_size(
                    center,
                    vec2(video_display_w, video_display_h),
                );

                // Draw video texture
                if let Some(ref texture) = self.video_texture {
                    ui.painter().image(
                        texture.id(),
                        video_screen_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    // Placeholder background
                    ui.painter().rect_filled(
                        video_screen_rect,
                        CornerRadius::ZERO,
                        Color32::from_rgb(15, 16, 18),
                    );
                    ui.painter().text(
                        video_screen_rect.center(),
                        Align2::CENTER_CENTER,
                        "Loading Frame...",
                        egui::FontId::proportional(16.0),
                        Color32::GRAY,
                    );
                }

                // Render Crop Overlay if enabled
                if self.crop_mode_enabled {
                    render_crop_overlay(
                        ui,
                        video_screen_rect,
                        &mut self.crop_state,
                        meta.width,
                        meta.height,
                    );
                }
            } else {
                // Empty state: Drag & Drop Dropzone
                render_empty_dropzone(ui, |app| {
                    app.open_file_dialog();
                }, self);
            }
        });

        // Render Export Modal dialog if open
        if let Some(ref meta) = self.metadata {
            let crop_pixels = if self.crop_mode_enabled && self.crop_state.is_cropped() {
                Some(self.crop_state.to_pixel_rect(meta.width, meta.height))
            } else {
                None
            };
            self.export_modal.render(
                ctx,
                meta,
                crop_pixels,
                self.trim_state.start_time,
                self.trim_state.end_time,
            );
        }
    }
}

fn render_empty_dropzone<F>(ui: &mut Ui, mut on_click_open: F, app: &mut VideoCropTrimApp)
where
    F: FnMut(&mut VideoCropTrimApp),
{
    let avail = ui.available_size();
    let center = ui.min_rect().min + avail / 2.0;

    let drop_box_rect = Rect::from_center_size(center, vec2(520.0, 290.0));
    let is_hovered = ui.rect_contains_pointer(drop_box_rect);

    let border_color = if !app.ffmpeg_available {
        Color32::from_rgb(220, 80, 80)
    } else if is_hovered {
        Color32::from_rgb(255, 190, 40)
    } else {
        Color32::from_rgb(60, 65, 78)
    };

    let bg_color = if !app.ffmpeg_available {
        Color32::from_rgb(38, 26, 28)
    } else if is_hovered {
        Color32::from_rgb(32, 35, 42)
    } else {
        Color32::from_rgb(24, 26, 31)
    };

    ui.painter().rect(
        drop_box_rect,
        egui::CornerRadius::same(12),
        bg_color,
        Stroke::new(2.0, border_color),
        egui::StrokeKind::Inside,
    );

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(drop_box_rect), |ui| {
        ui.vertical_centered(|ui| {
            if !app.ffmpeg_available {
                ui.add_space(35.0);
                ui.label(
                    RichText::new("⚠️")
                        .size(44.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("FFmpeg is not installed")
                        .strong()
                        .size(20.0)
                        .color(Color32::from_rgb(255, 120, 110)),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("FFmpeg is required to load and export videos.\nPlease install FFmpeg or add it to your PATH.")
                        .size(13.0)
                        .color(Color32::from_rgb(200, 190, 190)),
                );
                ui.add_space(20.0);

                let recheck_btn = ui.add_sized(
                    [160.0, 32.0],
                    egui::Button::new(
                        RichText::new("🔄 Recheck FFmpeg")
                            .strong()
                            .color(Color32::WHITE),
                    ),
                );

                if recheck_btn.clicked() {
                    app.recheck_ffmpeg();
                }
            } else {
                ui.add_space(45.0);
                ui.label(
                    RichText::new("🎬")
                        .size(48.0),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Drag & Drop Video Here")
                        .strong()
                        .size(20.0)
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Supports MP4 (H.264 / H.265), MOV, MKV, WebM, AVI...")
                        .size(13.0)
                        .color(Color32::from_rgb(160, 165, 175)),
                );
                ui.add_space(24.0);

                let open_btn = ui.add_sized(
                    [160.0, 32.0],
                    egui::Button::new(
                        RichText::new("Browse Files...")
                            .strong()
                            .color(Color32::WHITE),
                    ),
                );

                if open_btn.clicked() {
                    on_click_open(app);
                }
            }
        });
    });
}

