// src/matrix_character.rs
// MatrixCharacter: 32×32 pixel-art sprite themed after The Matrix.
// Idle: 2 frames (eyes open / blink).
// Walk: 4 frames (full stride cycle).

use crate::character::CharacterSprite;

// ─── Sprite dimensions ────────────────────────────────────────────────────────
pub const CHAR_W: i32 = 32;
pub const CHAR_H: i32 = 32;

// ─── Animation timing ────────────────────────────────────────────────────────
const IDLE_FRAME_MS: u32 = 500; // ms per idle frame
const WALK_FRAME_MS: u32 = 150; // ms per walk frame
const WALK_SPEED: i32 = 2;     // pixels moved per update() call (~400 px/s at 5ms tick)

// ─── Color palette (RGB565, LV_COLOR_16_SWAP=1) ──────────────────────────────
const fn rgb565_const(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g6 = (g >> 2) as u16;
    let b5 = (b >> 3) as u16;
    let rgb = (r5 << 11) | (g6 << 5) | b5;
    (rgb >> 8) | (rgb << 8)
}

const BLACK:        u16 = 0x0000;
const MATRIX_GREEN: u16 = rgb565_const(0x00, 0xFF, 0x41); // Matrix green #00FF41
const GREEN_DIM:    u16 = rgb565_const(0x00, 0x60, 0x20); // dimmed green (closed eye)
const COAT_DK:      u16 = rgb565_const(0x1A, 0x1A, 0x1A); // trench coat shadow
const COAT_MID:     u16 = rgb565_const(0x2E, 0x2E, 0x2E); // trench coat midtone
const COAT_HL:      u16 = rgb565_const(0x48, 0x48, 0x48); // trench coat highlight (center crease)
const SKIN:         u16 = rgb565_const(0xC8, 0x84, 0x5A); // warm skin
const HAIR:         u16 = rgb565_const(0x0C, 0x0C, 0x0C); // near-black hair

// ─── Pixel geometry helpers ───────────────────────────────────────────────────
const fn in_rect(x: i32, y: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    x >= x1 && x <= x2 && y >= y1 && y <= y2
}

const fn in_ellipse(x: i32, y: i32, cx: i32, cy: i32, a: i32, b: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    dx * dx * b * b + dy * dy * a * a <= a * a * b * b
}

// ─── Idle pixel art ───────────────────────────────────────────────────────────
// Front-facing Matrix agent/operative.
// 32×32; cx=16 (horizontal centre).
//
// Anatomy:
//   Rows  0–2  : dark hair, top of head
//   Rows  3–8  : face (skin ellipse), rows 5=eyes
//   Rows  9–10 : neck
//   Rows 11–13 : coat collar + shoulders
//   Rows 14–25 : coat body (subtle centre highlight)
//   Rows 26–31 : coat hem + legs (coat darker at bottom)
//
// `eyes_open`: true → MATRIX_GREEN eyes; false → dim slit (blink frame).
const fn idle_pixel(x: i32, y: i32, eyes_open: bool) -> u16 {
    let cx: i32 = 16;

    // Head: ellipse cx=16, cy=6, rx=6, ry=6
    let in_head = in_ellipse(x, y, cx, 6, 6, 6);

    // Hair covers top of head (y<=4) within the head ellipse
    let in_hair = in_head && y <= 4;

    // Face = head minus hair
    let in_face = in_head && y > 4;

    // Eyes: two 2-wide pixels at row 5, cx±4
    let in_left_eye  = y == 5 && (x == cx - 4 || x == cx - 3);
    let in_right_eye = y == 5 && (x == cx + 3 || x == cx + 4);
    let in_eyes = in_left_eye || in_right_eye;

    // Neck: slim column, rows 9-10
    let in_neck = in_rect(x, y, cx - 2, 9, cx + 2, 10);

    // Shoulders: rows 11-12, full width
    let in_shoulders = in_rect(x, y, cx - 10, 11, cx + 10, 12);

    // Collar V-shape inside shoulders: rows 11-13, narrower
    let in_collar = in_rect(x, y, cx - 4, 11, cx + 4, 13);

    // Coat body: rows 13-25, slightly flares outward toward bottom
    let coat_hw: i32 = if y <= 20 { 8 } else { 8 + (y - 20) / 3 };
    let in_coat = y >= 13 && y <= 25 && x >= cx - coat_hw && x <= cx + coat_hw;

    // Coat hem + legs: rows 26-31 (two dark columns for legs, coat between)
    let in_left_leg  = in_rect(x, y, cx - 7, 26, cx - 3, 31);
    let in_right_leg = in_rect(x, y, cx + 3, 26, cx + 7, 31);
    let in_coat_hem  = in_rect(x, y, cx - 2, 26, cx + 2, 29); // gap between legs = inner coat

    if in_hair {
        HAIR
    } else if in_face {
        if in_eyes {
            if eyes_open { MATRIX_GREEN } else { GREEN_DIM }
        } else {
            SKIN
        }
    } else if in_neck {
        SKIN
    } else if in_shoulders || in_collar || in_coat {
        // Centre crease highlight on the coat
        if x == cx { COAT_HL } else if x >= cx - 2 && x <= cx + 2 { COAT_MID } else { COAT_DK }
    } else if in_left_leg || in_right_leg || in_coat_hem {
        COAT_DK
    } else {
        BLACK
    }
}

// ─── Walk pixel art ───────────────────────────────────────────────────────────
// `step`: 0=left foot fwd, 1=mid, 2=right foot fwd, 3=mid
// Legs shift; coat bottom flares slightly with stride.
const fn walk_pixel(x: i32, y: i32, step: u8) -> u16 {
    let cx: i32 = 16;

    // Head + face same as idle frame A (eyes always open while walking)
    let in_head = in_ellipse(x, y, cx, 6, 6, 6);
    let in_hair = in_head && y <= 4;
    let in_face = in_head && y > 4;
    let in_left_eye  = y == 5 && (x == cx - 4 || x == cx - 3);
    let in_right_eye = y == 5 && (x == cx + 3 || x == cx + 4);
    let in_eyes = in_left_eye || in_right_eye;

    let in_neck      = in_rect(x, y, cx - 2, 9, cx + 2, 10);
    let in_shoulders = in_rect(x, y, cx - 10, 11, cx + 10, 12);
    let in_collar    = in_rect(x, y, cx - 4, 11, cx + 4, 13);

    let coat_hw: i32 = if y <= 20 { 8 } else { 8 + (y - 20) / 3 };
    let in_coat = y >= 13 && y <= 25 && x >= cx - coat_hw && x <= cx + coat_hw;

    // Leg offsets: positive = forward (toward viewer = lower y for back leg effect)
    // step 0: left leg forward (y shift -1 for left, +1 for right relative to idle)
    // step 2: right leg forward (mirror)
    // steps 1,3: mid-stride (feet level, slightly apart)
    let (ll_y_off, rl_y_off, ll_x_off, rl_x_off): (i32, i32, i32, i32) = match step {
        0 => (-1, 1, -1, 1),  // left fwd: left leg higher + shifted left
        1 => (0,  0,  0, 0),  // mid-stride level
        2 => (1, -1,  1, -1), // right fwd: mirror
        _ => (0,  0,  0, 0),  // mid-stride level
    };

    let in_left_leg  = y >= 26 && y <= 31
        && in_rect(x - ll_x_off, y + ll_y_off, cx - 7, 26, cx - 3, 31);
    let in_right_leg = y >= 26 && y <= 31
        && in_rect(x - rl_x_off, y + rl_y_off, cx + 3, 26, cx + 7, 31);
    let in_coat_hem  = in_rect(x, y, cx - 2, 26, cx + 2, 29);

    if in_hair { HAIR }
    else if in_face {
        if in_eyes { MATRIX_GREEN } else { SKIN }
    }
    else if in_neck      { SKIN }
    else if in_shoulders || in_collar || in_coat {
        if x == cx { COAT_HL } else if x >= cx - 2 && x <= cx + 2 { COAT_MID } else { COAT_DK }
    }
    else if in_left_leg || in_right_leg || in_coat_hem { COAT_DK }
    else { BLACK }
}

// ─── Frame builders ───────────────────────────────────────────────────────────
const fn make_idle_frame(eyes_open: bool) -> [u16; (CHAR_W * CHAR_H) as usize] {
    let mut p = [BLACK; (CHAR_W * CHAR_H) as usize];
    let mut y = 0i32;
    while y < CHAR_H {
        let mut x = 0i32;
        while x < CHAR_W {
            p[(y * CHAR_W + x) as usize] = idle_pixel(x, y, eyes_open);
            x += 1;
        }
        y += 1;
    }
    p
}

const fn make_walk_frame(step: u8) -> [u16; (CHAR_W * CHAR_H) as usize] {
    let mut p = [BLACK; (CHAR_W * CHAR_H) as usize];
    let mut y = 0i32;
    while y < CHAR_H {
        let mut x = 0i32;
        while x < CHAR_W {
            p[(y * CHAR_W + x) as usize] = walk_pixel(x, y, step);
            x += 1;
        }
        y += 1;
    }
    p
}

// ─── Static frame data (lives in flash) ──────────────────────────────────────
static IDLE_FRAME_0: [u16; (CHAR_W * CHAR_H) as usize] = make_idle_frame(true);
static IDLE_FRAME_1: [u16; (CHAR_W * CHAR_H) as usize] = make_idle_frame(false);
static WALK_FRAME_0: [u16; (CHAR_W * CHAR_H) as usize] = make_walk_frame(0);
static WALK_FRAME_1: [u16; (CHAR_W * CHAR_H) as usize] = make_walk_frame(1);
static WALK_FRAME_2: [u16; (CHAR_W * CHAR_H) as usize] = make_walk_frame(2);
static WALK_FRAME_3: [u16; (CHAR_W * CHAR_H) as usize] = make_walk_frame(3);

// ─── Animation state ─────────────────────────────────────────────────────────
enum AnimState {
    Idle,
    Walking { target_x: i32, target_y: i32 },
}

// ─── Struct ───────────────────────────────────────────────────────────────────
pub struct MatrixCharacter {
    // One image descriptor per frame, stored inside the leaked struct so that
    // their addresses remain stable (LVGL holds raw pointers).
    idle_dscs: [lvgl_sys::lv_img_dsc_t; 2],
    walk_dscs: [lvgl_sys::lv_img_dsc_t; 4],

    widget: *mut lvgl_sys::lv_obj_t,

    state:          AnimState,
    frame_idx:      usize,
    frame_timer_ms: u32,
    pos_x:          i32,
    pos_y:          i32,
}

// Safety: MatrixCharacter is only used on the single LVGL thread.
unsafe impl Send for MatrixCharacter {}
unsafe impl Sync for MatrixCharacter {}

// ─── Helper: build an lv_img_dsc_t from a pixel slice ────────────────────────
fn make_dsc(pixels: &'static [u16]) -> lvgl_sys::lv_img_dsc_t {
    let mut dsc = lvgl_sys::lv_img_dsc_t::default();
    dsc.header.set_cf(lvgl_sys::LV_IMG_CF_TRUE_COLOR as u32);
    dsc.header.set_w(CHAR_W as u32);
    dsc.header.set_h(CHAR_H as u32);
    dsc.data_size = (CHAR_W * CHAR_H * core::mem::size_of::<u16>() as i32) as u32;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}

impl MatrixCharacter {
    /// Create a new MatrixCharacter on `screen` at position `(x, y)`.
    /// The struct is Box::leaked so LVGL descriptor pointers remain valid.
    /// Returns a `'static` mutable reference — store in a static mut pointer.
    ///
    /// # Safety
    /// Must be called from the LVGL init block (single-threaded context).
    pub unsafe fn new(screen: *mut lvgl_sys::lv_obj_t, x: i32, y: i32) -> &'static mut Self {
        // Build image descriptors for all 6 frames.
        let idle_dscs = [
            make_dsc(&IDLE_FRAME_0),
            make_dsc(&IDLE_FRAME_1),
        ];
        let walk_dscs = [
            make_dsc(&WALK_FRAME_0),
            make_dsc(&WALK_FRAME_1),
            make_dsc(&WALK_FRAME_2),
            make_dsc(&WALK_FRAME_3),
        ];

        // Create the LVGL image widget.
        let widget = lvgl_sys::lv_img_create(screen);
        lvgl_sys::lv_obj_set_pos(widget, x as i16, y as i16);

        // Leak the struct so it has a stable address for LVGL.
        let this: &'static mut Self = Box::leak(Box::new(MatrixCharacter {
            idle_dscs,
            walk_dscs,
            widget,
            state: AnimState::Idle,
            frame_idx: 0,
            frame_timer_ms: 0,
            pos_x: x,
            pos_y: y,
        }));

        // Set initial image (idle frame 0).
        lvgl_sys::lv_img_set_src(
            this.widget,
            &this.idle_dscs[0] as *const lvgl_sys::lv_img_dsc_t as *const core::ffi::c_void,
        );

        this
    }
}
