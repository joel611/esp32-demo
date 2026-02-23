# Matrix Character Sprite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a `CharacterSprite` trait and `MatrixCharacter` concrete struct with idle (2-frame blink) and walk (4-frame cycle) animations on a 32×32 pixel art sprite themed after *The Matrix*.

**Architecture:** A `CharacterSprite` trait defines the public API (`walk_to`, `update`, `position`, `is_idle`). `MatrixCharacter` implements it: it holds LVGL image descriptors for each animation frame inside the leaked struct, and advances frames + position in `update()` called from the existing LVGL loop in `main.rs`. Pixel frames are generated at compile time via `const fn` pixel math (same pattern as `spaceship.rs`).

**Tech Stack:** Rust, LVGL 8.x via `lvgl_sys` FFI, `esp-idf-svc`, `const fn` pixel art, RGB565 with `LV_COLOR_16_SWAP=1`.

---

## Key Context Before Starting

- **No host test runner.** This is embedded. "Test" = `cargo build` (compile check) + `cargo espflash flash --monitor` (visual verify on device).
- **LVGL widget raw pointer lifetime.** The `MatrixCharacter` struct must be `Box::leak`ed so its address is stable — LVGL stores raw pointers to image descriptors inside it.
- **RGB565 byte-swap.** `LV_COLOR_16_SWAP=1` is active. Use `rgb565_const(r, g, b)` (same helper as `spaceship.rs`) for all colors.
- **`lv_img_set_src` signature.** Takes `*const c_void`. Cast the descriptor reference as `dsc as *const lvgl_sys::lv_img_dsc_t as *const core::ffi::c_void`.
- **`lv_obj_set_pos` signature.** Takes `i16` coords, not `i32`.
- **Working branch:** `feat/spaceship`. All files go under `src/`.
- **Existing analogues to read first:** `src/spaceship.rs` (pixel art pattern), `src/main.rs:128-164` (how `make_dsc` + timer callbacks work).

---

## Task 1: Create `src/character.rs` — the trait

**Files:**
- Create: `src/character.rs`
- Modify: `src/main.rs` (add `mod character;`)

### Step 1: Write the trait

Create `src/character.rs`:

```rust
// src/character.rs
// Abstract interface for animated, moveable pixel-art characters.

/// Implemented by any character that has idle and walk animations
/// and can be moved to a target position on screen.
pub trait CharacterSprite {
    /// Begin walking toward `(target_x, target_y)` (top-left of sprite).
    /// No-op if already at that position.
    fn walk_to(&mut self, target_x: i32, target_y: i32);

    /// Advance animation frames and position by `delta_ms` milliseconds.
    /// Call this every tick from the LVGL loop (typically every 5 ms).
    fn update(&mut self, delta_ms: u32);

    /// Current top-left position on screen.
    fn position(&self) -> (i32, i32);

    /// True when the character is not walking.
    fn is_idle(&self) -> bool;
}
```

### Step 2: Add `mod character;` to `main.rs`

In `src/main.rs`, after the existing `mod spaceship;` line, add:

```rust
mod character;
```

### Step 3: Compile check

```bash
cargo build 2>&1 | head -30
```

Expected: compiles cleanly (the trait has no implementation yet so no missing-impl errors).

### Step 4: Commit

```bash
git add src/character.rs src/main.rs
git commit -m "feat: add CharacterSprite trait"
```

---

## Task 2: Create `src/matrix_character.rs` — struct skeleton + color constants

**Files:**
- Create: `src/matrix_character.rs`
- Modify: `src/main.rs` (add `mod matrix_character;`)

### Step 1: Write the skeleton (no pixel art yet)

Create `src/matrix_character.rs`:

```rust
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
```

### Step 2: Add `mod matrix_character;` to `main.rs`

In `src/main.rs`, after `mod character;`:

```rust
mod matrix_character;
```

### Step 3: Compile check

```bash
cargo build 2>&1 | head -40
```

Expected: compiles (struct exists but no impl yet — expect "unused" warnings only, no errors).

### Step 4: Commit

```bash
git add src/matrix_character.rs src/main.rs
git commit -m "feat: MatrixCharacter struct skeleton + color palette"
```

---

## Task 3: Pixel art — idle frames (32×32)

**Files:**
- Modify: `src/matrix_character.rs` (add pixel art const fn below the color constants)

### Step 1: Add geometry helpers + idle pixel functions

Append to `src/matrix_character.rs` (before the `AnimState` enum):

```rust
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
```

### Step 2: Compile check

```bash
cargo build 2>&1 | head -40
```

Expected: builds cleanly (pixel data computes at compile time).

If you see `evaluation of constant value failed`: a const fn is using a feature not yet stable (closures, etc.). Check that only `if/else`, `while`, `let`, integer arithmetic, and the helper `const fn`s are used.

### Step 3: Commit

```bash
git add src/matrix_character.rs
git commit -m "feat: Matrix character pixel art (idle 2-frame, walk 4-frame)"
```

---

## Task 4: `MatrixCharacter::new()` constructor

**Files:**
- Modify: `src/matrix_character.rs` (add `impl MatrixCharacter`)

### Step 1: Add a `make_dsc` helper + `new()`

Append to `src/matrix_character.rs`:

```rust
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
```

### Step 2: Compile check

```bash
cargo build 2>&1 | head -40
```

Expected: clean build. The `CharacterSprite` trait is not yet implemented, so you may see "trait not implemented" if you've referenced it — that's fine at this step.

### Step 3: Commit

```bash
git add src/matrix_character.rs
git commit -m "feat: MatrixCharacter::new() constructor"
```

---

## Task 5: Implement `CharacterSprite` for `MatrixCharacter`

**Files:**
- Modify: `src/matrix_character.rs` (add `impl CharacterSprite`)

### Step 1: Add the trait implementation

Append to `src/matrix_character.rs`:

```rust
impl CharacterSprite for MatrixCharacter {
    fn walk_to(&mut self, target_x: i32, target_y: i32) {
        if target_x == self.pos_x && target_y == self.pos_y {
            return;
        }
        self.state = AnimState::Walking { target_x, target_y };
        self.frame_idx = 0;
        self.frame_timer_ms = 0;
    }

    fn update(&mut self, delta_ms: u32) {
        self.frame_timer_ms += delta_ms;

        match &self.state {
            AnimState::Idle => {
                if self.frame_timer_ms >= IDLE_FRAME_MS {
                    self.frame_timer_ms = 0;
                    self.frame_idx = (self.frame_idx + 1) % self.idle_dscs.len();
                    unsafe {
                        lvgl_sys::lv_img_set_src(
                            self.widget,
                            &self.idle_dscs[self.frame_idx] as *const lvgl_sys::lv_img_dsc_t
                                as *const core::ffi::c_void,
                        );
                    }
                }
            }

            AnimState::Walking { target_x, target_y } => {
                let (tx, ty) = (*target_x, *target_y);

                // Advance walk frame.
                if self.frame_timer_ms >= WALK_FRAME_MS {
                    self.frame_timer_ms = 0;
                    self.frame_idx = (self.frame_idx + 1) % self.walk_dscs.len();
                    unsafe {
                        lvgl_sys::lv_img_set_src(
                            self.widget,
                            &self.walk_dscs[self.frame_idx] as *const lvgl_sys::lv_img_dsc_t
                                as *const core::ffi::c_void,
                        );
                    }
                }

                // Move toward target.
                let dx = tx - self.pos_x;
                let dy = ty - self.pos_y;
                let dist_sq = dx * dx + dy * dy;
                let speed_sq = WALK_SPEED * WALK_SPEED;

                if dist_sq <= speed_sq {
                    // Arrived.
                    self.pos_x = tx;
                    self.pos_y = ty;
                    self.state = AnimState::Idle;
                    self.frame_idx = 0;
                    self.frame_timer_ms = 0;
                    unsafe {
                        lvgl_sys::lv_img_set_src(
                            self.widget,
                            &self.idle_dscs[0] as *const lvgl_sys::lv_img_dsc_t
                                as *const core::ffi::c_void,
                        );
                    }
                } else {
                    // Step proportionally toward target using integer sqrt approximation.
                    // ESP32-S3 has FPU; f32 sqrt is fine at runtime.
                    let dist = libm::sqrtf(dist_sq as f32) as i32;
                    self.pos_x += (dx * WALK_SPEED) / dist;
                    self.pos_y += (dy * WALK_SPEED) / dist;
                }

                unsafe {
                    lvgl_sys::lv_obj_set_pos(self.widget, self.pos_x as i16, self.pos_y as i16);
                }
            }
        }
    }

    fn position(&self) -> (i32, i32) {
        (self.pos_x, self.pos_y)
    }

    fn is_idle(&self) -> bool {
        matches!(self.state, AnimState::Idle)
    }
}
```

> **Note on `libm`:** We use `libm::sqrtf` for the distance calculation. If `libm` is not already in `Cargo.toml`, add it. Alternatively replace with an integer isqrt loop if you prefer no-f32 (but f32 is fine on ESP32-S3 which has FPU).
>
> To check if libm is available: `grep libm Cargo.toml`.
> If not present, add to `Cargo.toml`:
> ```toml
> [dependencies]
> libm = "0.2"
> ```

### Step 2: Compile check

```bash
cargo build 2>&1 | head -50
```

Expected: clean build. Fix any lifetime or type errors — the most common ones are:
- `*const c_void` vs `*const _`: use explicit cast chain `as *const lvgl_sys::lv_img_dsc_t as *const core::ffi::c_void`
- Borrow checker issues with `&self.state` while mutating `self`: clone the target values out of the enum before the mutable ops (the code above does this with `let (tx, ty) = (*target_x, *target_y)`)

### Step 3: Commit

```bash
git add src/matrix_character.rs Cargo.toml Cargo.lock
git commit -m "feat: implement CharacterSprite for MatrixCharacter"
```

---

## Task 6: Wire into `main.rs` and flash

**Files:**
- Modify: `src/main.rs`

### Step 1: Add static pointer + instantiate in init

In `src/main.rs`, after the existing `static mut BLINK_WIDGET`:

```rust
use crate::matrix_character::MatrixCharacter;
use crate::character::CharacterSprite;

// MatrixCharacter — lives for program lifetime (Box::leaked in ::new())
static mut MATRIX_CHAR: *mut MatrixCharacter = core::ptr::null_mut();
```

Inside the `unsafe { ... }` init block in `main()`, after the blink widget setup (before the timer creates):

```rust
// ── Matrix character ─────────────────────────────────────────────────────────
// Spawn at centre of screen; call walk_to() to move it.
MATRIX_CHAR = MatrixCharacter::new(SCREEN1, 200, 200);
log::info!("MatrixCharacter created at (200, 200)");
```

### Step 2: Call `update()` in the LVGL loop

In the LVGL loop (the `loop { ... }` block), after `lv_timer_handler()`:

```rust
// Advance MatrixCharacter animation (idle blink / walk frames + position).
unsafe { (*MATRIX_CHAR).update(5); }
```

### Step 3: (Optional smoke test) trigger a walk in init

Immediately after creating the character in the init block, add a test walk to verify movement works:

```rust
// Quick smoke test: walk from (200,200) to (100,350).
(*MATRIX_CHAR).walk_to(100, 350);
```

Remove this line after visual verification.

### Step 4: Compile check

```bash
cargo build 2>&1 | head -50
```

Expected: clean build.

### Step 5: Flash and visually verify

```bash
cargo espflash flash --monitor
```

Visual checks:
- [ ] A 32×32 Matrix character appears on the spaceship screen at (200, 200)
- [ ] The character walks toward (100, 350) then stops
- [ ] While walking: coat figure strides (legs alternate)
- [ ] While idle: eyes blink every ~500ms (green → dim → green)
- [ ] Existing crew/commander/blink animations still run normally

### Step 6: Commit

```bash
git add src/main.rs
git commit -m "feat: wire MatrixCharacter into main LVGL loop"
```

---

## Task 7: Clean up smoke test + final commit

### Step 1: Remove the smoke-test `walk_to` line from `main.rs`

Remove the temporary `(*MATRIX_CHAR).walk_to(100, 350);` from the init block.

### Step 2: Final compile + flash

```bash
cargo build && cargo espflash flash --monitor
```

Verify character is stationary at (200, 200) blinking idle.

### Step 3: Final commit

```bash
git add src/main.rs
git commit -m "chore: remove MatrixCharacter smoke test walk_to"
```

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `const fn` compile error about closures | Used a closure in const context | Replace with `if/else` + helper `const fn` |
| Character invisible | `lv_img_set_src` called with null or wrong pointer | Check that `&this.idle_dscs[0]` address is correct; add `log::info!` to verify non-null |
| Character flickers badly | `update()` called too frequently or `lv_img_set_src` every tick | Only call `lv_img_set_src` when frame index changes (the timer check does this) |
| Walk stops immediately | `dist_sq <= speed_sq` true on first step | Check that `(tx, ty) != (pos_x, pos_y)` — `walk_to` no-ops if already at target |
| `libm` not found | Not in Cargo.toml | `grep libm Cargo.toml`; if missing add `libm = "0.2"` to `[dependencies]` |
| CMake stale cache build error | C component cache stale after changes | `rm -rf target/xtensa-esp32s3-espidf/debug/build/esp-idf-sys-*/` then rebuild |
