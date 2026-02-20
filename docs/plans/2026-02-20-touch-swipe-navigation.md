# Touch Swipe Navigation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add FT3168 touch driver and swipe-to-navigate between two LVGL screens on the ESP32-S3 466×466 AMOLED display.

**Architecture:** Port the FT3168 I2C touch driver from the worktree as a minimal Rust module. Poll touch in the main loop and store state in atomics. Register an LVGL pointer input device that reads those atomics. LVGL gesture detection fires `LV_EVENT_GESTURE` on the screen object; the callback calls `lv_scr_load_anim()` to switch between two pre-created screen objects.

**Tech Stack:** Rust (no_std-ish on ESP-IDF), lvgl-sys 0.6.2 (LVGL 8.x unsafe bindings), esp-idf-hal I2cDriver, FT3168 touch controller (I2C 0x38, SDA=GPIO47, SCL=GPIO48, 600 kHz).

---

## Background / Key Facts

- `src/main.rs` is the only Rust source file. Everything runs in a single-threaded main loop.
- The LVGL loop is: `lv_tick_inc(5)` → `lv_timer_handler()` → `sleep(5ms)`. The indev read callback is called **synchronously inside `lv_timer_handler()`**, so polling touch before that call and storing in atomics is safe.
- lvgl-sys constants use short names: `LV_ALIGN_CENTER` not `lv_align_t_LV_ALIGN_CENTER`.
- `lv_obj_create(null_mut())` creates a new LVGL screen object (parent = NULL).
- FT3168 returns raw X/Y only; gesture direction is computed by LVGL from the drag trajectory.
- The default LVGL gesture limit is 50px (`LV_INDEV_DEF_GESTURE_LIMIT`). A horizontal drag > 50px fires `LV_EVENT_GESTURE`.
- Labels are not clickable by default in LVGL 8.x, so touching the screen (even over a label) sends the touch to the screen object — no `LV_OBJ_FLAG_GESTURE_BUBBLE` needed.
- `lv_scr_load_anim(..., auto_del=false)` keeps the old screen alive so we can swap back.

---

## Task 1: Add `src/ft3168.rs` — FT3168 touch driver

**Files:**
- Create: `src/ft3168.rs`

**Step 1: Create the module**

```rust
// src/ft3168.rs
use esp_idf_svc::hal::i2c::I2cDriver;

const ADDR: u8 = 0x38;

pub struct Ft3168<'d> {
    i2c: I2cDriver<'d>,
}

impl<'d> Ft3168<'d> {
    pub fn new(i2c: I2cDriver<'d>) -> Self {
        Self { i2c }
    }

    /// Switch FT3168 to normal mode. Call once after power-on.
    /// The 200 ms delay lets the controller stabilise.
    pub fn init(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        std::thread::sleep(std::time::Duration::from_millis(200));
        self.i2c.write(ADDR, &[0x00, 0x00], 1000)?;
        Ok(())
    }

    /// Returns `Some((x, y))` if a finger is currently touching the screen,
    /// `None` if no touch is active.
    pub fn read_touch(&mut self) -> Result<Option<(u16, u16)>, esp_idf_svc::sys::EspError> {
        let mut count = [0u8; 1];
        self.i2c.write_read(ADDR, &[0x02], &mut count, 1000)?;
        if count[0] == 0 {
            return Ok(None);
        }

        let mut buf = [0u8; 4];
        self.i2c.write_read(ADDR, &[0x03], &mut buf, 1000)?;

        // Register layout: buf[0] bits[3:0] = X[11:8], buf[1] = X[7:0]
        //                  buf[2] bits[3:0] = Y[11:8], buf[3] = Y[7:0]
        let x = (((buf[0] & 0x0F) as u16) << 8) | buf[1] as u16;
        let y = (((buf[2] & 0x0F) as u16) << 8) | buf[3] as u16;

        Ok(Some((x.min(465), y.min(465))))
    }
}
```

**Step 2: Build to confirm it compiles**

```bash
cargo build 2>&1 | grep -E "error|warning.*ft3168" | head -20
```

Expected: `error[E0583]: file not found for module 'ft3168'` — because we haven't added `mod ft3168;` yet. That's fine, we do it in Task 2.

**Step 3: Commit**

```bash
git add src/ft3168.rs
git commit -m "feat: add FT3168 I2C touch driver module"
```

---

## Task 2: Wire I2C + FT3168 init into `src/main.rs`

**Files:**
- Modify: `src/main.rs`

**Context:** We add I2C peripheral init, construct `Ft3168`, call `init()`, then add `mod ft3168;`. The FT3168 init happens **before** `lcd_driver_init` (or after — order doesn't matter; they're independent buses).

**Step 1: Add imports at the top of `src/main.rs`**

Add these lines after the existing `use std::time::Duration;`:

```rust
mod ft3168;

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
```

**Step 2: Add global touch state atomics**

Add these `static` declarations before `fn main()`:

```rust
// Touch state written by the main loop, read by the LVGL indev callback.
// Both run on the same thread (indev cb is called inside lv_timer_handler),
// so Relaxed ordering is sufficient.
static TOUCH_X: AtomicI32 = AtomicI32::new(0);
static TOUCH_Y: AtomicI32 = AtomicI32::new(0);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);
```

**Step 3: Initialize I2C and FT3168 in `fn main()`**

Add this block **before** the `lcd_driver_init()` call (around line 33):

```rust
// ── 0. Touch controller init ──────────────────────────────────────────────
let peripherals = Peripherals::take().unwrap();
let i2c_config = I2cConfig::new().baudrate(Hertz(600_000));
let i2c = I2cDriver::new(
    peripherals.i2c0,
    peripherals.pins.gpio47, // SDA
    peripherals.pins.gpio48, // SCL
    &i2c_config,
)
.unwrap();
let mut ft3168 = ft3168::Ft3168::new(i2c);
ft3168.init().expect("FT3168 init failed");
log::info!("FT3168 touch controller ready");
```

**Step 4: Build to confirm it compiles**

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: clean build (no errors). If you see `error[E0433]: failed to resolve: use of undeclared crate or module`, check the import paths — esp-idf-hal is re-exported under `esp_idf_svc::hal`.

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: initialize I2C and FT3168 touch controller"
```

---

## Task 3: Poll touch in main loop + register LVGL input device

**Files:**
- Modify: `src/main.rs`

**Step 1: Add the LVGL indev read callback**

Add this function **before** `fn main()` (alongside the existing `lvgl_flush_cb`):

```rust
/// LVGL input device read callback. Called by lv_timer_handler() on every tick.
/// Reads touch state from the atomics updated in the main loop.
unsafe extern "C" fn lvgl_touch_cb(
    _drv: *mut lvgl_sys::lv_indev_drv_t,
    data: *mut lvgl_sys::lv_indev_data_t,
) {
    if TOUCH_PRESSED.load(Ordering::Relaxed) {
        (*data).point.x = TOUCH_X.load(Ordering::Relaxed) as lvgl_sys::lv_coord_t;
        (*data).point.y = TOUCH_Y.load(Ordering::Relaxed) as lvgl_sys::lv_coord_t;
        (*data).state = lvgl_sys::LV_INDEV_STATE_PRESSED as u8;
    } else {
        (*data).state = lvgl_sys::LV_INDEV_STATE_RELEASED as u8;
    }
}
```

**Step 2: Register the input device in the `unsafe` init block**

Add this after the `lv_disp_drv_register` call (after line 67 in the current file, inside the `unsafe { ... }` block that sets up LVGL):

```rust
// ── 6. Input device (touch) ───────────────────────────────────────────────
let indev_drv: &'static mut lvgl_sys::lv_indev_drv_t =
    Box::leak(Box::new(core::mem::zeroed()));
lvgl_sys::lv_indev_drv_init(indev_drv);
indev_drv.type_ = lvgl_sys::LV_INDEV_TYPE_POINTER as u8;
indev_drv.read_cb = Some(lvgl_touch_cb);
lvgl_sys::lv_indev_drv_register(indev_drv);
log::info!("LVGL touch input registered");
```

> **Note on field name:** The `type` field in `lv_indev_drv_t` is named `type_` in Rust (since `type` is a keyword). If you get a compile error, check the field name with `cargo doc --open` or search the bindings.

**Step 3: Add touch polling at the TOP of the main loop**

Replace the existing loop:

```rust
loop {
    unsafe {
        lvgl_sys::lv_tick_inc(5);
        lvgl_sys::lv_timer_handler();
    }
    std::thread::sleep(Duration::from_millis(5));
}
```

With:

```rust
loop {
    // Poll touch BEFORE lv_timer_handler() so the indev callback
    // (called inside lv_timer_handler) sees the current state.
    match ft3168.read_touch() {
        Ok(Some((x, y))) => {
            TOUCH_X.store(x as i32, Ordering::Relaxed);
            TOUCH_Y.store(y as i32, Ordering::Relaxed);
            TOUCH_PRESSED.store(true, Ordering::Relaxed);
        }
        _ => {
            TOUCH_PRESSED.store(false, Ordering::Relaxed);
        }
    }

    unsafe {
        lvgl_sys::lv_tick_inc(5);
        lvgl_sys::lv_timer_handler();
    }
    std::thread::sleep(Duration::from_millis(5));
}
```

**Step 4: Build**

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: clean build.

**Step 5: Flash and verify touch is polled without crash**

```bash
cargo espflash flash --monitor 2>&1 | head -40
```

Expected log output:
```
=== LVGL display test ===
FT3168 touch controller ready
lcd_driver_init OK
LVGL display registered
LVGL touch input registered
UI created
Entering LVGL loop
```

If `FT3168 init failed` panics, check I2C wiring and that `Peripherals::take()` wasn't already called elsewhere.

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire FT3168 touch polling to LVGL input device"
```

---

## Task 4: Create two screens and add swipe gesture navigation

**Files:**
- Modify: `src/main.rs`

**Step 1: Add static screen pointer storage**

Add before `fn main()`:

```rust
// Screen object pointers. Written once during init, read by gesture callback.
static mut SCREEN1: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut SCREEN2: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
```

**Step 2: Add the gesture event callback**

Add before `fn main()`:

```rust
/// Gesture event callback attached to both screens.
/// Swipe LEFT  → load screen 2 (if on screen 1).
/// Swipe RIGHT → load screen 1 (if on screen 2).
unsafe extern "C" fn gesture_cb(e: *mut lvgl_sys::lv_event_t) {
    let indev = lvgl_sys::lv_indev_get_act();
    if indev.is_null() {
        return;
    }
    let dir = lvgl_sys::lv_indev_get_gesture_dir(indev);
    let active = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());

    // LV_DIR_LEFT = 0x04, LV_DIR_RIGHT = 0x08 in LVGL 8.x
    // Verify with: grep -r "LV_DIR_LEFT" target/xtensa*/debug/build/lvgl-sys-*/out/bindings.rs
    if dir == lvgl_sys::LV_DIR_LEFT as u8 && active == SCREEN1 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN2,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_LEFT,
            300,
            0,
            false,
        );
    } else if dir == lvgl_sys::LV_DIR_RIGHT as u8 && active == SCREEN2 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN1,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_RIGHT,
            300,
            0,
            false,
        );
    }
}
```

> **Note on constant names:** `lv_scr_load_anim_t` enum values may be prefixed (`lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_LEFT`) or unprefixed (`LV_SCR_LOAD_ANIM_MOVE_LEFT`) depending on the bindgen version. Check the bindings file:
> ```bash
> grep "SCR_LOAD_ANIM_MOVE" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs | head -5
> ```
> Use whichever form compiles.

> **Note on `lv_dir_t` values:** Same — check the bindings:
> ```bash
> grep "LV_DIR_LEFT\|LV_DIR_RIGHT" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs | head -5
> ```

**Step 3: Replace the UI setup block with two-screen setup**

Find and replace the current `// ── 6. Simple UI: centered label` section (currently lines 70–75):

```rust
// ── 7. Two-screen UI ─────────────────────────────────────────────────────
// Screen 1: the default screen LVGL created when the display was registered.
SCREEN1 = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());

// Dark background for screen 1
lvgl_sys::lv_obj_set_style_bg_color(
    SCREEN1,
    lvgl_sys::_LV_COLOR_MAKE(0x10, 0x10, 0x10),
    lvgl_sys::LV_STATE_DEFAULT as u32,
);

let label1 = lvgl_sys::lv_label_create(SCREEN1);
lvgl_sys::lv_label_set_text(label1, b"Screen 1\0".as_ptr() as *const i8);
lvgl_sys::lv_obj_align(label1, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 0);

// Screen 2: new screen object (parent = null → creates a standalone screen)
SCREEN2 = lvgl_sys::lv_obj_create(core::ptr::null_mut());

// Blue background for screen 2 so it's visually distinct
lvgl_sys::lv_obj_set_style_bg_color(
    SCREEN2,
    lvgl_sys::_LV_COLOR_MAKE(0x00, 0x30, 0x80),
    lvgl_sys::LV_STATE_DEFAULT as u32,
);

let label2 = lvgl_sys::lv_label_create(SCREEN2);
lvgl_sys::lv_label_set_text(label2, b"Screen 2\0".as_ptr() as *const i8);
lvgl_sys::lv_obj_align(label2, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 0);

// Attach gesture callbacks — LVGL sends LV_EVENT_GESTURE to the screen
// when a drag exceeds LV_INDEV_DEF_GESTURE_LIMIT (default 50px).
lvgl_sys::lv_obj_add_event_cb(
    SCREEN1,
    Some(gesture_cb),
    lvgl_sys::LV_EVENT_GESTURE as u32,
    core::ptr::null_mut(),
);
lvgl_sys::lv_obj_add_event_cb(
    SCREEN2,
    Some(gesture_cb),
    lvgl_sys::LV_EVENT_GESTURE as u32,
    core::ptr::null_mut(),
);

log::info!("Two screens created, gesture callbacks attached");
```

> **Note on `lv_obj_set_style_bg_color` / `_LV_COLOR_MAKE`:** If these don't compile, check bindings:
> ```bash
> grep "lv_obj_set_style_bg_color\|_LV_COLOR_MAKE" \
>   target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs | head -5
> ```
> Alternatively, skip the background color for now and just test with labels on the default grey background.

**Step 4: Build**

```bash
cargo build 2>&1 | grep -E "^error" | head -30
```

Fix any constant name mismatches using the grep commands in the notes above. The most likely issue is `lv_scr_load_anim_t_*` vs `LV_SCR_LOAD_ANIM_*` and `LV_DIR_LEFT` type/value.

**Step 5: Flash and test**

```bash
cargo espflash flash --monitor
```

Verification checklist:
1. Screen 1 shows "Screen 1" label — ✓ display still works
2. Swipe left (drag finger from right to left > 50px) → Screen 2 appears with "Screen 2"
3. Swipe right on Screen 2 → Screen 1 reappears
4. Swiping right on Screen 1 does nothing (no crash)
5. Swiping left on Screen 2 does nothing (no crash)

**If swipe does nothing:** The gesture may not be reaching the screen. Debug by temporarily adding an `LV_EVENT_ALL` handler that logs all events. Also verify the indev is registered by adding `log::info!("indev registered: {:?}", !indev_drv as *mut _ as usize == 0)`.

**If display goes blank on swipe:** The `auto_del=false` parameter may be getting ignored, or the screen2 pointer in the static is stale. Add a log in `gesture_cb` to confirm `dir` and pointer values.

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add two-screen LVGL UI with touch swipe gesture navigation"
```

---

## Constant Name Lookup Reference

If any constant doesn't compile, run these to find the correct name in the generated bindings:

```bash
# Gesture direction constants
grep "LV_DIR" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs | grep -v "//"

# Screen load animation constants
grep "SCR_LOAD_ANIM" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs

# Indev type constants
grep "LV_INDEV_TYPE" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs

# Event code constants
grep "LV_EVENT_GESTURE\|LV_EVENT_ALL" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs

# Indev state constants
grep "LV_INDEV_STATE" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs

# lv_indev_get_gesture_dir return type
grep "lv_indev_get_gesture_dir" target/xtensa-esp32s3-espidf/debug/build/lvgl-sys-*/out/bindings.rs
```
