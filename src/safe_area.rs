/// Safe area utilities for the 466×466 circular AMOLED display.
///
/// The display is a circle with radius 233, centered at (233, 233).
/// Pixels outside this circle are physically invisible even though the
/// framebuffer is 466×466.
pub const DISPLAY_W: i32 = 466;
pub const DISPLAY_H: i32 = 466;
pub const DISPLAY_CX: i32 = DISPLAY_W / 2; // 233
pub const DISPLAY_CY: i32 = DISPLAY_H / 2; // 233
pub const DISPLAY_R: i32 = DISPLAY_W / 2;  // 233

/// Returns `true` if the point (x, y) lies within the circular display.
pub fn point_in_display(x: i32, y: i32) -> bool {
    let dx = x - DISPLAY_CX;
    let dy = y - DISPLAY_CY;
    dx * dx + dy * dy <= DISPLAY_R * DISPLAY_R
}

/// Returns `true` if the entire rectangle [x, x+w) × [y, y+h) lies within
/// the display circle. All four corners must be inside.
pub fn rect_in_display(x: i32, y: i32, w: i32, h: i32) -> bool {
    point_in_display(x,         y)
        && point_in_display(x + w - 1, y)
        && point_in_display(x,         y + h - 1)
        && point_in_display(x + w - 1, y + h - 1)
}

/// Clamp a rectangle's (x, y) so that it fits entirely within the circular display.
///
/// Strategy: compute the maximum radius from the display center within which the
/// widget center can move while keeping all corners inside the circle, then clamp
/// the widget center to that inner circle.
///
/// If the widget is too large to fit at all (its bounding circle exceeds the
/// display radius), it is simply centered on the display.
pub fn clamp_rect_to_display(x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    // Fast path: already fits.
    if rect_in_display(x, y, w, h) {
        return (x, y);
    }

    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;

    // Half-diagonal of the widget (radius of its bounding circle).
    let widget_r = (hw * hw + hh * hh).sqrt();

    // Maximum distance from display center to widget center so all corners stay in.
    let max_center_r = DISPLAY_R as f32 - widget_r;

    if max_center_r < 0.0 {
        // Widget cannot fit; center it as best-effort.
        return (DISPLAY_CX - w / 2, DISPLAY_CY - h / 2);
    }

    // Current widget center.
    let wcx = x as f32 + hw;
    let wcy = y as f32 + hh;

    let dx = wcx - DISPLAY_CX as f32;
    let dy = wcy - DISPLAY_CY as f32;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist <= max_center_r {
        // Fits after all (floating-point rounding may differ from integer corners check).
        return (x, y);
    }

    // Scale the offset vector down to the boundary of the valid inner circle.
    let scale = max_center_r / dist;
    let new_wcx = DISPLAY_CX as f32 + dx * scale;
    let new_wcy = DISPLAY_CY as f32 + dy * scale;

    ((new_wcx - hw) as i32, (new_wcy - hh) as i32)
}
