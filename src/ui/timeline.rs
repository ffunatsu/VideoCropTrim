use egui::{
    pos2, vec2, Color32, CursorIcon, Rect, Sense, Stroke, Ui,
};
use crate::utils::time::format_time;

#[derive(Debug, Clone)]
pub struct TrimState {
    pub start_time: f64,
    pub end_time: f64,
    pub current_time: f64,
    pub duration: f64,
    active_drag: TimelineDrag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimelineDrag {
    None,
    InHandle,
    OutHandle,
    Playhead,
}

impl Default for TrimState {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            end_time: 0.0,
            current_time: 0.0,
            duration: 0.0,
            active_drag: TimelineDrag::None,
        }
    }
}

impl TrimState {
    pub fn new(duration: f64) -> Self {
        Self {
            start_time: 0.0,
            end_time: duration.max(0.0),
            current_time: 0.0,
            duration: duration.max(0.0),
            active_drag: TimelineDrag::None,
        }
    }

    pub fn reset(&mut self, duration: f64) {
        self.duration = duration.max(0.0);
        self.start_time = 0.0;
        self.end_time = self.duration;
        self.current_time = 0.0;
    }

    pub fn set_in_to_current(&mut self) {
        self.start_time = self.current_time.min(self.end_time - 0.05).max(0.0);
    }

    pub fn set_out_to_current(&mut self) {
        self.end_time = self.current_time.max(self.start_time + 0.05).min(self.duration);
    }

    pub fn is_trimmed(&self) -> bool {
        self.start_time > 0.05 || (self.duration - self.end_time).abs() > 0.05
    }

    pub fn trim_duration(&self) -> f64 {
        (self.end_time - self.start_time).max(0.0)
    }

    pub fn seek_to(&mut self, time: f64) {
        self.current_time = time.clamp(0.0, self.duration);
    }

    pub fn step_frames(&mut self, frames: i32, fps: f64) {
        let frame_duration = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };
        let new_time = self.current_time + (frames as f64 * frame_duration);
        self.seek_to(new_time);
    }

    pub fn step_seconds(&mut self, seconds: f64) {
        let new_time = self.current_time + seconds;
        self.seek_to(new_time);
    }
}

pub fn render_timeline(
    ui: &mut Ui,
    trim_state: &mut TrimState,
    _fps: f64,
) -> bool {
    let mut time_changed = false;
    let duration = trim_state.duration;
    if duration <= 0.0 {
        return false;
    }

    let track_height = 28.0;
    let (response, painter) = ui.allocate_painter(
        vec2(ui.available_width(), track_height),
        Sense::click_and_drag(),
    );
    let rect = response.rect;

    let handle_width = 12.0;
    let track_margin = 6.0;
    let track_rect = Rect::from_min_max(
        pos2(rect.min.x + track_margin, rect.min.y + 4.0),
        pos2(rect.max.x - track_margin, rect.max.y - 4.0),
    );

    let time_to_x = |t: f64| -> f32 {
        let frac = (t / duration).clamp(0.0, 1.0) as f32;
        track_rect.min.x + frac * track_rect.width()
    };

    let x_to_time = |x: f32| -> f64 {
        let frac = ((x - track_rect.min.x) / track_rect.width()).clamp(0.0, 1.0) as f64;
        frac * duration
    };

    let in_x = time_to_x(trim_state.start_time);
    let out_x = time_to_x(trim_state.end_time);
    let playhead_x = time_to_x(trim_state.current_time);

    let in_handle_rect = Rect::from_center_size(
        pos2(in_x, track_rect.center().y),
        vec2(handle_width, track_rect.height() + 8.0),
    );
    let out_handle_rect = Rect::from_center_size(
        pos2(out_x, track_rect.center().y),
        vec2(handle_width, track_rect.height() + 8.0),
    );
    let playhead_rect = Rect::from_center_size(
        pos2(playhead_x, track_rect.center().y),
        vec2(14.0, track_rect.height() + 10.0),
    );

    // Pointer interactions
    let pointer_pos = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));

    if let Some(pos) = pointer_pos {
        let hover_in = in_handle_rect.contains(pos);
        let hover_out = out_handle_rect.contains(pos);
        let hover_play = playhead_rect.contains(pos);

        if trim_state.active_drag == TimelineDrag::None {
            if hover_in || hover_out {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            } else if hover_play {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }

        if ui.input(|i| i.pointer.any_pressed()) && response.hovered() {
            if hover_play {
                trim_state.active_drag = TimelineDrag::Playhead;
            } else if hover_in {
                trim_state.active_drag = TimelineDrag::InHandle;
            } else if hover_out {
                trim_state.active_drag = TimelineDrag::OutHandle;
            } else {
                // Clicked on track: jump playhead
                let clicked_time = x_to_time(pos.x);
                trim_state.seek_to(clicked_time);
                trim_state.active_drag = TimelineDrag::Playhead;
                time_changed = true;
            }
        }
    }

    if ui.input(|i| i.pointer.any_released()) {
        trim_state.active_drag = TimelineDrag::None;
    }

    if trim_state.active_drag != TimelineDrag::None {
        if let Some(pos) = pointer_pos {
            let t = x_to_time(pos.x);
            match trim_state.active_drag {
                TimelineDrag::InHandle => {
                    trim_state.start_time = t.min(trim_state.end_time - 0.05).max(0.0);
                    trim_state.current_time = trim_state.current_time.clamp(trim_state.start_time, trim_state.end_time);
                    time_changed = true;
                }
                TimelineDrag::OutHandle => {
                    trim_state.end_time = t.max(trim_state.start_time + 0.05).min(duration);
                    trim_state.current_time = trim_state.current_time.clamp(trim_state.start_time, trim_state.end_time);
                    time_changed = true;
                }
                TimelineDrag::Playhead => {
                    trim_state.seek_to(t);
                    time_changed = true;
                }
                TimelineDrag::None => {}
            }
        }
    }

    // DRAWING:
    // 1. Inactive background track
    painter.rect_filled(
        track_rect,
        egui::CornerRadius::same(4),
        Color32::from_rgb(35, 38, 45),
    );

    // 2. Active trim selection region
    let active_rect = Rect::from_min_max(
        pos2(in_x, track_rect.min.y),
        pos2(out_x, track_rect.max.y),
    );
    let trim_highlight_color = Color32::from_rgb(255, 180, 0); // QuickTime yellow accent
    painter.rect_filled(
        active_rect,
        egui::CornerRadius::same(2),
        Color32::from_rgb(55, 62, 75),
    );

    // Trim top/bottom boundary bars
    painter.line_segment(
        [pos2(in_x, track_rect.min.y), pos2(out_x, track_rect.min.y)],
        Stroke::new(3.0, trim_highlight_color),
    );
    painter.line_segment(
        [pos2(in_x, track_rect.max.y), pos2(out_x, track_rect.max.y)],
        Stroke::new(3.0, trim_highlight_color),
    );

    // 3. In Handle ([)
    let in_color = trim_highlight_color;
    painter.rect_filled(
        in_handle_rect,
        egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 },
        in_color,
    );
    // Draw grabber dots inside In Handle
    let in_center = in_handle_rect.center();
    painter.circle_filled(pos2(in_center.x, in_center.y - 4.0), 1.5, Color32::BLACK);
    painter.circle_filled(pos2(in_center.x, in_center.y + 4.0), 1.5, Color32::BLACK);

    // 4. Out Handle (])
    let out_color = trim_highlight_color;
    painter.rect_filled(
        out_handle_rect,
        egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 },
        out_color,
    );
    let out_center = out_handle_rect.center();
    painter.circle_filled(pos2(out_center.x, out_center.y - 4.0), 1.5, Color32::BLACK);
    painter.circle_filled(pos2(out_center.x, out_center.y + 4.0), 1.5, Color32::BLACK);

    // 5. Playhead indicator (Red / White needle)
    let playhead_color = Color32::from_rgb(235, 75, 75);
    painter.line_segment(
        [pos2(playhead_x, rect.min.y), pos2(playhead_x, rect.max.y)],
        Stroke::new(2.0, playhead_color),
    );
    // Playhead top cap / thumb
    let thumb_pts = vec![
        pos2(playhead_x - 5.0, rect.min.y),
        pos2(playhead_x + 5.0, rect.min.y),
        pos2(playhead_x + 5.0, rect.min.y + 7.0),
        pos2(playhead_x, rect.min.y + 12.0),
        pos2(playhead_x - 5.0, rect.min.y + 7.0),
    ];
    painter.add(egui::Shape::convex_polygon(
        thumb_pts,
        playhead_color,
        Stroke::new(1.0, Color32::WHITE),
    ));

    time_changed
}

pub fn render_transport_controls(
    ui: &mut Ui,
    trim_state: &mut TrimState,
    is_playing: &mut bool,
    fps: f64,
) -> bool {
    let mut seek_requested = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

        // Time display left (Current / Total)
        ui.label(
            egui::RichText::new(format_time(trim_state.current_time))
                .strong()
                .size(15.0)
                .color(Color32::WHITE),
        );
        ui.label(
            egui::RichText::new(format!("/ {}", format_time(trim_state.duration)))
                .size(13.0)
                .color(Color32::from_rgb(160, 160, 160)),
        );

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        // Jump to Start / In Point
        if ui.button("⏮").on_hover_text("Jump to Trim Start").clicked() {
            trim_state.seek_to(trim_state.start_time);
            seek_requested = true;
        }

        // Step -1s
        if ui.button("⏪").on_hover_text("Step -1.0s (Shift+Left)").clicked() {
            trim_state.step_seconds(-1.0);
            seek_requested = true;
        }

        // Step -1 frame
        if ui.button("◀|").on_hover_text("Step -1 frame (Left)").clicked() {
            trim_state.step_frames(-1, fps);
            seek_requested = true;
        }

        // Play / Pause button
        let play_btn_text = if *is_playing { " ⏸ " } else { " ▶ " };
        let play_btn = ui.add_sized([44.0, 24.0], egui::Button::new(
            egui::RichText::new(play_btn_text).strong().size(15.0)
        ));
        if play_btn.on_hover_text("Play / Pause (Space)").clicked() {
            *is_playing = !*is_playing;
        }

        // Step +1 frame
        if ui.button("|▶").on_hover_text("Step +1 frame (Right)").clicked() {
            trim_state.step_frames(1, fps);
            seek_requested = true;
        }

        // Step +1s
        if ui.button("⏩").on_hover_text("Step +1.0s (Shift+Right)").clicked() {
            trim_state.step_seconds(1.0);
            seek_requested = true;
        }

        // Jump to End / Out Point
        if ui.button("⏭").on_hover_text("Jump to Trim End").clicked() {
            trim_state.seek_to(trim_state.end_time);
            seek_requested = true;
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        // In / Out point setter buttons
        if ui.button("[ In").on_hover_text("Set Trim In point to playhead (I)").clicked() {
            trim_state.set_in_to_current();
        }
        if ui.button("Out ]").on_hover_text("Set Trim Out point to playhead (O)").clicked() {
            trim_state.set_out_to_current();
        }
        if trim_state.is_trimmed() {
            if ui.button("↺ Reset Trim").on_hover_text("Reset Trim to full video").clicked() {
                let dur = trim_state.duration;
                trim_state.reset(dur);
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Trim duration label
            let trim_len = trim_state.trim_duration();
            ui.label(
                egui::RichText::new(format!("Selected: {}", format_time(trim_len)))
                    .size(13.0)
                    .color(Color32::from_rgb(255, 200, 80)),
            );
        });
    });

    seek_requested
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_state_logic() {
        let mut trim = TrimState::new(60.0);
        assert_eq!(trim.duration, 60.0);
        assert_eq!(trim.start_time, 0.0);
        assert_eq!(trim.end_time, 60.0);
        assert!(!trim.is_trimmed());

        trim.seek_to(15.0);
        trim.set_in_to_current();
        assert_eq!(trim.start_time, 15.0);
        assert!(trim.is_trimmed());

        trim.seek_to(45.0);
        trim.set_out_to_current();
        assert_eq!(trim.end_time, 45.0);
        assert_eq!(trim.trim_duration(), 30.0);

        trim.step_frames(30, 30.0); // +1 second
        assert_eq!(trim.current_time, 46.0);

        trim.step_seconds(-2.0);
        assert_eq!(trim.current_time, 44.0);
    }
}

