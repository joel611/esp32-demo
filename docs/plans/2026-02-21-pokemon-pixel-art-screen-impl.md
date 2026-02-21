# Pokemon Pixel Art Screen Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace Screen 1 with a Pokemon-style pixel art scene — centered 64×64 RGB565 sprite, 2-frame idle animation toggling every 600ms, and a yellow "PIKACHU" name label below.

**Architecture:** Add `src/sprites.rs` for static pixel arrays. Construct `lv_img_dsc_t` descriptors in `main()` via `Box::leak` (same pattern as `lv_disp_drv_t`). Store raw pointers in `static mut` globals for the timer callback (same pattern as `SCREEN1`/`SCREEN2`). `lv_timer_create` drives the 600ms frame toggle — no changes to the LVGL loop required.

**Tech Stack:** Rust, lvgl-sys 0.6.2 (LVGL 8.x FFI), ESP32-S3, no new Cargo dependencies.

**Design doc:** `docs/plans/2026-02-20-pokemon-pixel-art-screen-design.md`

---

## Verified API signatures (from generated bindings)

```
lv_img_create(parent: *mut lv_obj_t) -> *mut lv_obj_t
lv_img_set_src(obj: *mut lv_obj_t, src: *const cty::c_void)
lv_timer_create(timer_xcb: lv_timer_cb_t, period: u32, user_data: *mut cty::c_void)
  where lv_timer_cb_t = Option<unsafe extern "C" fn(*mut lv_timer_t)>
lv_obj_set_style_text_color(obj, color, selector: u32)
lv_img_dsc_t { header: lv_img_header_t, data_size: u32, data: *const u8 }
  .header.set_cf(val: u32)   LV_IMG_CF_TRUE_COLOR = 4
  .header.set_w(val: u32)
  .header.set_h(val: u32)
  lv_img_dsc_t::default() → zero-initialized (safe, Default impl exists)
```

---

### Task 1: Create `src/sprites.rs` with placeholder pixel data

**Files:**
- Create: `src/sprites.rs`
- Modify: `src/main.rs` (add `mod sprites;`)

**Step 1: Add the module declaration to `src/main.rs`**

After the `mod safe_area;` line (line 4), add:

```rust
mod sprites;
```

**Step 2: Create `src/sprites.rs`**

```rust
// Two RGB565 64×64 frames for Pikachu idle animation.
//
// LV_COLOR_16_SWAP=1 is active. The internal lv_color_t byte layout is:
//   bits  0–2 : green_h (upper 3 of 6-bit green)
//   bits  3–7 : red (5 bits)
//   bits  8–12: blue (5 bits)
//   bits 13–15: green_l (lower 3 of 6-bit green)
//
// Yellow (R=255, G=255, B=0): green_h=7|red=31|blue=0|green_l=7
//   = 0b111_11111_00000_111 → as u16 little-endian = 0xE0FF
//
// PLACEHOLDER DATA: solid color fills so widget layout can be verified before
// adding real art. Replace with proper 64×64 Pikachu sprite later.
// To convert PNG → RGB565 array: use https://lvgl.io/tools/imageconverter
// (select CF_TRUE_COLOR, 16-bit color, swap bytes).

pub static PIKACHU_FRAME_A: [u16; 4096] = [0xE0FF; 4096]; // yellow — idle pose
pub static PIKACHU_FRAME_B: [u16; 4096] = [0xC0FF; 4096]; // amber — blink/twitch
```

**Step 3: Build to verify the new module compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: `Finished` or only warnings (no errors).

**Step 4: Commit**

```bash
git add src/sprites.rs src/main.rs
git commit -m "feat: add sprites module with placeholder 64x64 RGB565 frame data"
```

---

### Task 2: Add animation statics and timer callback to `src/main.rs`

**Files:**
- Modify: `src/main.rs` (near lines 32–34, and before `fn main()`)

**Step 1: Add `static mut` pointers for the animation state**

After the existing `static mut SCREEN2` declaration (line 33), add:

```rust
// Animation state — written once during init, accessed only on the main thread.
static mut IMG_WIDGET: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut FRAME_IDX: u8 = 0;
static mut IMG_A_DSC: *const lvgl_sys::lv_img_dsc_t = core::ptr::null();
static mut IMG_B_DSC: *const lvgl_sys::lv_img_dsc_t = core::ptr::null();
```

**Step 2: Add the timer callback before `fn main()`**

After the `gesture_cb` function (before `fn main()`), add:

```rust
/// LVGL timer callback — fires every 600 ms to toggle the sprite frame.
/// Called by lv_timer_handler() on the main thread; all statics are single-thread.
unsafe extern "C" fn anim_timer_cb(_timer: *mut lvgl_sys::lv_timer_t) {
    FRAME_IDX = 1 - FRAME_IDX;
    let src = if FRAME_IDX == 0 { IMG_A_DSC } else { IMG_B_DSC };
    lvgl_sys::lv_img_set_src(IMG_WIDGET, src as *const _);
}
```

**Step 3: Build to verify no errors**

```bash
cargo build 2>&1 | head -30
```

Expected: compiles cleanly.

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add animation statics and timer callback for sprite frame toggle"
```

---

### Task 3: Add `make_img_dsc` helper and rewrite Screen 1 body

**Files:**
- Modify: `src/main.rs` (before `fn main()`, and inside the `unsafe` block ~lines 181–191)

**Step 1: Add the `make_img_dsc` helper before `fn main()`**

```rust
/// Construct an lv_img_dsc_t pointing at a static 64×64 RGB565 pixel array.
/// The 'static lifetime guarantees the pointer outlives LVGL's display lifetime.
fn make_img_dsc(pixels: &'static [u16; 4096]) -> lvgl_sys::lv_img_dsc_t {
    let mut dsc = lvgl_sys::lv_img_dsc_t::default();
    dsc.header.set_cf(lvgl_sys::LV_IMG_CF_TRUE_COLOR as u32); // = 4
    dsc.header.set_w(64);
    dsc.header.set_h(64);
    dsc.data_size = (64 * 64 * core::mem::size_of::<u16>()) as u32;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}
```

**Step 2: Replace the Screen 1 setup block**

The current Screen 1 block (inside the `unsafe { ... }` in `main()`) looks like:

```rust
// Dark background for screen 1
lvgl_sys::lv_obj_set_style_bg_color(
    SCREEN1,
    lvgl_sys::_LV_COLOR_MAKE(0x10, 0x10, 0x10),
    lvgl_sys::LV_STATE_DEFAULT,
);

let label1 = lvgl_sys::lv_label_create(SCREEN1);
lvgl_sys::lv_label_set_text(label1, b"Screen 1\0".as_ptr() as *const i8);
lvgl_sys::lv_obj_align(label1, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 0);
```

Replace it entirely with:

```rust
// ── Screen 1: Pokemon sprite scene ───────────────────────────────────────

// Black background
lvgl_sys::lv_obj_set_style_bg_color(
    SCREEN1,
    lvgl_sys::_LV_COLOR_MAKE(0x00, 0x00, 0x00),
    lvgl_sys::LV_STATE_DEFAULT,
);

// Construct and leak image descriptors (LVGL holds raw pointer; must be 'static)
let img_a = Box::leak(Box::new(make_img_dsc(&sprites::PIKACHU_FRAME_A)));
let img_b = Box::leak(Box::new(make_img_dsc(&sprites::PIKACHU_FRAME_B)));
IMG_A_DSC = img_a as *const _;
IMG_B_DSC = img_b as *const _;

// Image widget — centered, shifted up 30 px so the name label fits below
let img_widget = lvgl_sys::lv_img_create(SCREEN1);
lvgl_sys::lv_img_set_src(img_widget, img_a as *const _);
lvgl_sys::lv_obj_align(img_widget, lvgl_sys::LV_ALIGN_CENTER as u8, 0, -30);
IMG_WIDGET = img_widget;

// Name label — yellow, 50 px below the center point
let name_label = lvgl_sys::lv_label_create(SCREEN1);
lvgl_sys::lv_label_set_text(name_label, b"PIKACHU\0".as_ptr() as *const i8);
lvgl_sys::lv_obj_set_style_text_color(
    name_label,
    lvgl_sys::_LV_COLOR_MAKE(0xFF, 0xFF, 0x00),
    lvgl_sys::LV_STATE_DEFAULT,
);
lvgl_sys::lv_obj_align(name_label, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 50);

// 600 ms timer — drives the idle animation frame toggle
lvgl_sys::lv_timer_create(Some(anim_timer_cb), 600, core::ptr::null_mut());
```

**Step 3: Build**

```bash
cargo build 2>&1
```

Expected: `Finished` with 0 errors.

> **Troubleshooting:**
> - `method not found: set_cf/set_w/set_h` → run:
>   `grep -E "fn (set_cf|set_w|set_h)" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs`
>   and update `make_img_dsc` with the correct method names.
> - `lv_img_create not found` → check:
>   `grep "lv_img_create\|lv_img_set_src" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs`
> - `type mismatch on set_cf` → cast the constant: `lvgl_sys::LV_IMG_CF_TRUE_COLOR as u32`

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: Screen 1 Pokemon pixel art scene with 2-frame idle animation"
```

---

### Task 4: Flash and visual verification

**Step 1: Flash and monitor**

```bash
cargo espflash flash --monitor
```

Expected serial output (order may vary):

```
=== LVGL display test ===
FT3168 touch controller ready
lcd_driver_init OK
LVGL display registered
LVGL touch input registered
Two screens created, gesture callbacks attached
Entering LVGL loop
```

**Step 2: Visual check on the display**

| What to look for | Expected |
|---|---|
| Screen 1 background | Pure black |
| Sprite area | 64×64 yellow square, centered and shifted ~30 px above center |
| Frame toggle | Color shifts between yellow and amber every ~600 ms |
| Name label | "PIKACHU" in yellow, ~50 px below center |
| Swipe left | Transitions to Screen 2 (blue background, unchanged) |
| Swipe right on Screen 2 | Transitions back to Screen 1 |

**Step 3: Commit any fixes found during visual check**

```bash
git add -p
git commit -m "fix: <describe fix>"
```

---

## Replacing placeholder art with real Pikachu sprites

When ready to use proper pixel art:

1. Create a 64×64 PNG in any pixel art editor (Aseprite, Piskel, etc.)
2. Convert to RGB565 with byte-swap at https://lvgl.io/tools/imageconverter
   (select: CF_TRUE_COLOR, 16-bit color, swap bytes = ON)
3. Copy the generated `uint8_t` array, reinterpret as `u16` values, paste into `PIKACHU_FRAME_A`
4. Create a second pose (e.g., eyes closed or ear down) and populate `PIKACHU_FRAME_B`
5. `cargo build && cargo espflash flash --monitor`

No changes to `main.rs` are needed — only `src/sprites.rs`.
