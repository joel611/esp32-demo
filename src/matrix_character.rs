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
