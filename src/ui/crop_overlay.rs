use egui::{
    pos2, vec2, Color32, CursorIcon, Pos2, Rect, Response, Sense, Stroke, Ui,
};
use crate::video::CropRectPixels;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectRatioPreset {
    Free,
    Original,
    Ratio1x1,
    Ratio16x9,
    Ratio9x16,
    Ratio4x3,
    Ratio3x2,
    Ratio21x9,
    Ratio4x5,
}

impl AspectRatioPreset {
    pub fn label(&self) -> &'static str {
        match self {
            AspectRatioPreset::Free => "Freeform",
            AspectRatioPreset::Original => "Original",
            AspectRatioPreset::Ratio1x1 => "1:1 Square",
            AspectRatioPreset::Ratio16x9 => "16:9 Landscape",
            AspectRatioPreset::Ratio9x16 => "9:16 Shorts/Reels",
            AspectRatioPreset::Ratio4x3 => "4:3 Classic",
            AspectRatioPreset::Ratio3x2 => "3:2 Photo",
            AspectRatioPreset::Ratio21x9 => "21:9 Ultrawide",
            AspectRatioPreset::Ratio4x5 => "4:5 Portrait",
        }
    }

    pub fn ratio(&self, original_ratio: f32) -> Option<f32> {
        match self {
            AspectRatioPreset::Free => None,
            AspectRatioPreset::Original => Some(original_ratio),
            AspectRatioPreset::Ratio1x1 => Some(1.0),
            AspectRatioPreset::Ratio16x9 => Some(16.0 / 9.0),
            AspectRatioPreset::Ratio9x16 => Some(9.0 / 16.0),
            AspectRatioPreset::Ratio4x3 => Some(4.0 / 3.0),
            AspectRatioPreset::Ratio3x2 => Some(3.0 / 2.0),
            AspectRatioPreset::Ratio21x9 => Some(21.0 / 9.0),
            AspectRatioPreset::Ratio4x5 => Some(4.0 / 5.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragHandle {
    None,
    Move,
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Debug, Clone)]
pub struct CropState {
    /// Normalized crop rectangle: [0.0, 1.0] relative to video bounds
    pub norm_rect: Rect,
    pub aspect_preset: AspectRatioPreset,
    active_drag: DragHandle,
    drag_start_pos: Pos2,
    initial_norm_rect: Rect,
}

impl Default for CropState {
    fn default() -> Self {
        Self {
            norm_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            aspect_preset: AspectRatioPreset::Free,
            active_drag: DragHandle::None,
            drag_start_pos: Pos2::ZERO,
            initial_norm_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        }
    }
}

impl CropState {
    pub fn reset(&mut self) {
        self.norm_rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        self.aspect_preset = AspectRatioPreset::Free;
    }

    pub fn is_cropped(&self) -> bool {
        let r = self.norm_rect;
        r.min.x > 0.005 || r.min.y > 0.005 || r.max.x < 0.995 || r.max.y < 0.995
    }

    pub fn set_preset(&mut self, preset: AspectRatioPreset, video_w: u32, video_h: u32) {
        self.aspect_preset = preset;
        if video_w == 0 || video_h == 0 {
            return;
        }

        let orig_ratio = video_w as f32 / video_h as f32;
        if let Some(target_ratio) = preset.ratio(orig_ratio) {
            self.apply_aspect_ratio(target_ratio, video_w, video_h);
        }
    }

    pub fn apply_aspect_ratio(&mut self, target_ratio: f32, video_w: u32, video_h: u32) {
        let center = self.norm_rect.center();
        let video_aspect = video_w as f32 / video_h as f32;

        let mut norm_w = self.norm_rect.width();
        let mut norm_h = norm_w * (video_aspect / target_ratio);

        if norm_h > 1.0 {
            norm_h = 1.0;
            norm_w = norm_h * (target_ratio / video_aspect);
        }

        let min_x = (center.x - norm_w / 2.0).clamp(0.0, 1.0 - norm_w);
        let min_y = (center.y - norm_h / 2.0).clamp(0.0, 1.0 - norm_h);

        self.norm_rect = Rect::from_min_size(pos2(min_x, min_y), vec2(norm_w, norm_h));
    }

    pub fn to_pixel_rect(&self, video_w: u32, video_h: u32) -> CropRectPixels {
        let vw = video_w as f32;
        let vh = video_h as f32;

        let mut x = (self.norm_rect.min.x * vw).round() as u32;
        let mut y = (self.norm_rect.min.y * vh).round() as u32;
        let mut w = (self.norm_rect.width() * vw).round() as u32;
        let mut h = (self.norm_rect.height() * vh).round() as u32;

        // Bound to video dimensions
        x = x.min(video_w.saturating_sub(2));
        y = y.min(video_h.saturating_sub(2));
        w = w.clamp(2, video_w.saturating_sub(x));
        h = h.clamp(2, video_h.saturating_sub(y));

        CropRectPixels {
            x,
            y,
            width: w,
            height: h,
        }
    }

    pub fn set_from_pixels(&mut self, px: CropRectPixels, video_w: u32, video_h: u32) {
        if video_w == 0 || video_h == 0 {
            return;
        }
        let min_x = px.x as f32 / video_w as f32;
        let min_y = px.y as f32 / video_h as f32;
        let max_x = (px.x + px.width) as f32 / video_w as f32;
        let max_y = (px.y + px.height) as f32 / video_h as f32;

        self.norm_rect = Rect::from_min_max(
            pos2(min_x.clamp(0.0, 1.0), min_y.clamp(0.0, 1.0)),
            pos2(max_x.clamp(0.0, 1.0), max_y.clamp(0.0, 1.0)),
        );
    }
}

pub fn render_crop_overlay(
    ui: &mut Ui,
    video_screen_rect: Rect,
    crop_state: &mut CropState,
    video_w: u32,
    video_h: u32,
) -> Response {
    let id = ui.make_persistent_id("crop_overlay");
    let response = ui.interact(video_screen_rect, id, Sense::click_and_drag());

    let painter = ui.painter_at(video_screen_rect);
    let orig_ratio = if video_h > 0 {
        video_w as f32 / video_h as f32
    } else {
        1.0
    };

    // Calculate screen space crop rectangle
    let screen_crop_min = pos2(
        video_screen_rect.min.x + crop_state.norm_rect.min.x * video_screen_rect.width(),
        video_screen_rect.min.y + crop_state.norm_rect.min.y * video_screen_rect.height(),
    );
    let screen_crop_max = pos2(
        video_screen_rect.min.x + crop_state.norm_rect.max.x * video_screen_rect.width(),
        video_screen_rect.min.y + crop_state.norm_rect.max.y * video_screen_rect.height(),
    );
    let screen_crop_rect = Rect::from_min_max(screen_crop_min, screen_crop_max);

    let handle_size = 14.0;
    let handle_touch_radius = 16.0;

    // Handle mouse interaction
    let pointer_pos = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));

    if let Some(pos) = pointer_pos {
        let handles = [
            (DragHandle::TopLeft, screen_crop_rect.left_top(), CursorIcon::ResizeNorthWest),
            (DragHandle::TopRight, screen_crop_rect.right_top(), CursorIcon::ResizeNorthEast),
            (DragHandle::BottomRight, screen_crop_rect.right_bottom(), CursorIcon::ResizeSouthEast),
            (DragHandle::BottomLeft, screen_crop_rect.left_bottom(), CursorIcon::ResizeSouthWest),
            (DragHandle::Top, pos2(screen_crop_rect.center().x, screen_crop_rect.top()), CursorIcon::ResizeNorth),
            (DragHandle::Bottom, pos2(screen_crop_rect.center().x, screen_crop_rect.bottom()), CursorIcon::ResizeSouth),
            (DragHandle::Left, pos2(screen_crop_rect.left(), screen_crop_rect.center().y), CursorIcon::ResizeWest),
            (DragHandle::Right, pos2(screen_crop_rect.right(), screen_crop_rect.center().y), CursorIcon::ResizeEast),
        ];

        let mut hovered_handle = DragHandle::None;
        let mut hovered_cursor = CursorIcon::Default;

        for (h_type, h_pos, cursor) in &handles {
            if pos.distance(*h_pos) <= handle_touch_radius {
                hovered_handle = *h_type;
                hovered_cursor = *cursor;
                break;
            }
        }

        if hovered_handle == DragHandle::None && screen_crop_rect.contains(pos) {
            hovered_handle = DragHandle::Move;
            hovered_cursor = CursorIcon::Grab;
        }

        if crop_state.active_drag == DragHandle::None {
            if hovered_handle != DragHandle::None {
                ui.ctx().set_cursor_icon(hovered_cursor);
            }
        }

        if ui.input(|i| i.pointer.any_pressed()) && response.hovered() {
            crop_state.active_drag = hovered_handle;
            crop_state.drag_start_pos = pos;
            crop_state.initial_norm_rect = crop_state.norm_rect;
        }
    }

    if ui.input(|i| i.pointer.any_released()) {
        crop_state.active_drag = DragHandle::None;
    }

    if crop_state.active_drag != DragHandle::None {
        if let Some(pos) = pointer_pos {
            let delta_screen = pos - crop_state.drag_start_pos;
            let delta_norm = vec2(
                delta_screen.x / video_screen_rect.width(),
                delta_screen.y / video_screen_rect.height(),
            );

            let init = crop_state.initial_norm_rect;
            let mut new_rect = init;

            match crop_state.active_drag {
                DragHandle::Move => {
                    let w = init.width();
                    let h = init.height();
                    let min_x = (init.min.x + delta_norm.x).clamp(0.0, 1.0 - w);
                    let min_y = (init.min.y + delta_norm.y).clamp(0.0, 1.0 - h);
                    new_rect = Rect::from_min_size(pos2(min_x, min_y), vec2(w, h));
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                }
                DragHandle::TopLeft => {
                    let new_min_x = (init.min.x + delta_norm.x).clamp(0.0, init.max.x - 0.05);
                    let new_min_y = (init.min.y + delta_norm.y).clamp(0.0, init.max.y - 0.05);
                    new_rect = Rect::from_min_max(pos2(new_min_x, new_min_y), init.max);
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNorthWest);
                }
                DragHandle::TopRight => {
                    let new_max_x = (init.max.x + delta_norm.x).clamp(init.min.x + 0.05, 1.0);
                    let new_min_y = (init.min.y + delta_norm.y).clamp(0.0, init.max.y - 0.05);
                    new_rect = Rect::from_min_max(pos2(init.min.x, new_min_y), pos2(new_max_x, init.max.y));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNorthEast);
                }
                DragHandle::BottomRight => {
                    let new_max_x = (init.max.x + delta_norm.x).clamp(init.min.x + 0.05, 1.0);
                    let new_max_y = (init.max.y + delta_norm.y).clamp(init.min.y + 0.05, 1.0);
                    new_rect = Rect::from_min_max(init.min, pos2(new_max_x, new_max_y));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeSouthEast);
                }
                DragHandle::BottomLeft => {
                    let new_min_x = (init.min.x + delta_norm.x).clamp(0.0, init.max.x - 0.05);
                    let new_max_y = (init.max.y + delta_norm.y).clamp(init.min.y + 0.05, 1.0);
                    new_rect = Rect::from_min_max(pos2(new_min_x, init.min.y), pos2(init.max.x, new_max_y));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeSouthWest);
                }
                DragHandle::Top => {
                    let new_min_y = (init.min.y + delta_norm.y).clamp(0.0, init.max.y - 0.05);
                    new_rect = Rect::from_min_max(pos2(init.min.x, new_min_y), init.max);
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNorth);
                }
                DragHandle::Bottom => {
                    let new_max_y = (init.max.y + delta_norm.y).clamp(init.min.y + 0.05, 1.0);
                    new_rect = Rect::from_min_max(init.min, pos2(init.max.x, new_max_y));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeSouth);
                }
                DragHandle::Left => {
                    let new_min_x = (init.min.x + delta_norm.x).clamp(0.0, init.max.x - 0.05);
                    new_rect = Rect::from_min_max(pos2(new_min_x, init.min.y), init.max);
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeWest);
                }
                DragHandle::Right => {
                    let new_max_x = (init.max.x + delta_norm.x).clamp(init.min.x + 0.05, 1.0);
                    new_rect = Rect::from_min_max(init.min, pos2(new_max_x, init.max.y));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeEast);
                }
                DragHandle::None => {}
            }

            if let Some(target_ratio) = crop_state.aspect_preset.ratio(orig_ratio) {
                if crop_state.active_drag != DragHandle::Move && crop_state.active_drag != DragHandle::None {
                    let center = new_rect.center();
                    let cur_w = new_rect.width();
                    let cur_h = cur_w * (orig_ratio / target_ratio);
                    if cur_h <= 1.0 {
                        let min_x = (center.x - cur_w / 2.0).clamp(0.0, 1.0 - cur_w);
                        let min_y = (center.y - cur_h / 2.0).clamp(0.0, 1.0 - cur_h);
                        new_rect = Rect::from_min_size(pos2(min_x, min_y), vec2(cur_w, cur_h));
                    }
                }
            }

            crop_state.norm_rect = new_rect;
        }
    }

    // DRAWING:
    // 1. Dim background mask around crop rect
    let mask_color = Color32::from_black_alpha(150);

    // Top rect
    let top_mask = Rect::from_min_max(
        video_screen_rect.min,
        pos2(video_screen_rect.max.x, screen_crop_rect.min.y),
    );
    painter.rect_filled(top_mask, egui::CornerRadius::ZERO, mask_color);

    // Bottom rect
    let bot_mask = Rect::from_min_max(
        pos2(video_screen_rect.min.x, screen_crop_rect.max.y),
        video_screen_rect.max,
    );
    painter.rect_filled(bot_mask, egui::CornerRadius::ZERO, mask_color);

    // Left rect
    let left_mask = Rect::from_min_max(
        pos2(video_screen_rect.min.x, screen_crop_rect.min.y),
        pos2(screen_crop_rect.min.x, screen_crop_rect.max.y),
    );
    painter.rect_filled(left_mask, egui::CornerRadius::ZERO, mask_color);

    // Right rect
    let right_mask = Rect::from_min_max(
        pos2(screen_crop_rect.max.x, screen_crop_rect.min.y),
        pos2(video_screen_rect.max.x, screen_crop_rect.max.y),
    );
    painter.rect_filled(right_mask, egui::CornerRadius::ZERO, mask_color);

    // 2. Rule of thirds grid lines
    let grid_stroke = Stroke::new(1.0, Color32::from_white_alpha(70));
    let third_w = screen_crop_rect.width() / 3.0;
    let third_h = screen_crop_rect.height() / 3.0;

    for i in 1..=2 {
        let x = screen_crop_rect.min.x + third_w * i as f32;
        painter.line_segment(
            [pos2(x, screen_crop_rect.min.y), pos2(x, screen_crop_rect.max.y)],
            grid_stroke,
        );
        let y = screen_crop_rect.min.y + third_h * i as f32;
        painter.line_segment(
            [pos2(screen_crop_rect.min.x, y), pos2(screen_crop_rect.max.x, y)],
            grid_stroke,
        );
    }

    // 3. White outer crop border
    painter.rect_stroke(
        screen_crop_rect,
        egui::CornerRadius::ZERO,
        Stroke::new(1.5, Color32::from_rgb(255, 255, 255)),
        egui::StrokeKind::Inside,
    );

    // 4. Handles
    let handle_fill = Color32::WHITE;
    let handle_stroke = Stroke::new(1.0, Color32::BLACK);

    // 4 corner handles
    let corner_len = 16.0;
    let corner_thick = 3.0;
    let corner_stroke = Stroke::new(corner_thick, Color32::WHITE);

    // Top-Left Corner
    let tl = screen_crop_rect.left_top();
    painter.line_segment([tl, pos2(tl.x + corner_len, tl.y)], corner_stroke);
    painter.line_segment([tl, pos2(tl.x, tl.y + corner_len)], corner_stroke);

    // Top-Right Corner
    let tr = screen_crop_rect.right_top();
    painter.line_segment([tr, pos2(tr.x - corner_len, tr.y)], corner_stroke);
    painter.line_segment([tr, pos2(tr.x, tr.y + corner_len)], corner_stroke);

    // Bottom-Right Corner
    let br = screen_crop_rect.right_bottom();
    painter.line_segment([br, pos2(br.x - corner_len, br.y)], corner_stroke);
    painter.line_segment([br, pos2(br.x, br.y - corner_len)], corner_stroke);

    // Bottom-Left Corner
    let bl = screen_crop_rect.left_bottom();
    painter.line_segment([bl, pos2(bl.x + corner_len, bl.y)], corner_stroke);
    painter.line_segment([bl, pos2(bl.x, bl.y - corner_len)], corner_stroke);

    // Edge handles (pill/circle in center of edges)
    let edge_positions = [
        pos2(screen_crop_rect.center().x, screen_crop_rect.top()),
        pos2(screen_crop_rect.center().x, screen_crop_rect.bottom()),
        pos2(screen_crop_rect.left(), screen_crop_rect.center().y),
        pos2(screen_crop_rect.right(), screen_crop_rect.center().y),
    ];
    for p in edge_positions {
        painter.circle(p, 4.0, handle_fill, handle_stroke);
    }

    // 5. Dimension badge (e.g. 1080 x 1080)
    let px = crop_state.to_pixel_rect(video_w, video_h);
    let badge_text = format!("{} × {} px", px.width, px.height);
    let text_pos = if screen_crop_rect.top() - video_screen_rect.top() > 30.0 {
        pos2(screen_crop_rect.center().x, screen_crop_rect.top() - 12.0)
    } else {
        pos2(screen_crop_rect.center().x, screen_crop_rect.top() + 18.0)
    };

    let font_id = egui::FontId::proportional(12.0);
    let text_color = Color32::from_rgb(240, 240, 240);
    let badge_rect = Rect::from_center_size(text_pos, vec2(110.0, 20.0));
    painter.rect_filled(badge_rect, egui::CornerRadius::same(4), Color32::from_black_alpha(200));
    painter.text(
        text_pos,
        egui::Align2::CENTER_CENTER,
        badge_text,
        font_id,
        text_color,
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_presets() {
        let mut state = CropState::default();
        assert!(!state.is_cropped());

        state.set_preset(AspectRatioPreset::Ratio1x1, 1920, 1080);
        assert!(state.is_cropped());
        let px = state.to_pixel_rect(1920, 1080);
        assert_eq!(px.width, 1080);
        assert_eq!(px.height, 1080);
    }

    #[test]
    fn test_crop_pixel_mapping() {
        let mut state = CropState::default();
        state.norm_rect = Rect::from_min_max(pos2(0.25, 0.25), pos2(0.75, 0.75));
        let px = state.to_pixel_rect(1920, 1080);
        assert_eq!(px.x, 480);
        assert_eq!(px.y, 270);
        assert_eq!(px.width, 960);
        assert_eq!(px.height, 540);
    }
}

