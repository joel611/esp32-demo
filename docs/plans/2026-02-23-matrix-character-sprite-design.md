# Matrix Character Sprite Design

**Date:** 2026-02-23
**Branch:** feat/spaceship

## Problem

The project currently has sprite animation scattered as loose globals and LVGL timer callbacks
in `main.rs`. There is no reusable abstraction for animated characters with movement. Adding
new characters requires duplicating the same unsafe globals + callback pattern.

## Goal

Create a reusable `CharacterSprite` trait and a `MatrixCharacter` concrete implementation
with idle and walk animations on a 32×32 pixel art sprite, themed after the movie *The Matrix*.

---

## Design

### Architecture

```
src/character.rs          — CharacterSprite trait (abstract interface)
src/matrix_character.rs   — MatrixCharacter struct, sprite pixel art, trait impl
```

### `CharacterSprite` trait (`src/character.rs`)

```rust
pub trait CharacterSprite {
    /// Begin walking toward (target_x, target_y). No-op if already there.
    fn walk_to(&mut self, target_x: i32, target_y: i32);

    /// Advance animation and position. Call every tick (delta_ms ≈ 5ms).
    fn update(&mut self, delta_ms: u32);

    /// Current top-left position on screen.
    fn position(&self) -> (i32, i32);

    /// True when the character is in the Idle state.
    fn is_idle(&self) -> bool;
}
```

### Animation State Machine

```
        walk_to(x,y)
Idle ─────────────────► Walking
  ▲                         │
  └─────────────────────────┘
     arrived at target
```

- **Idle**: cycles through `idle_frames` every `IDLE_FRAME_MS` (default 500ms)
- **Walking**: cycles through `walk_frames` every `WALK_FRAME_MS` (default 150ms),
  moves `WALK_SPEED_PX` pixels/tick toward target; on arrival → Idle

### `MatrixCharacter` struct (`src/matrix_character.rs`)

```rust
pub struct MatrixCharacter {
    idle_frames: &'static [&'static [u16]],   // 32×32 = 1024 u16 each
    walk_frames: &'static [&'static [u16]],
    img_dsc:     &'static mut lv_img_dsc_t,   // Box::leaked
    widget:      *mut lv_obj_t,               // LVGL image widget
    state:       AnimState,
    frame_idx:   usize,
    frame_timer_ms: u32,
    pos_x:       i32,
    pos_y:       i32,
}

enum AnimState {
    Idle,
    Walking { target_x: i32, target_y: i32 },
}
```

**Constructor:**
```rust
impl MatrixCharacter {
    pub fn new(screen: *mut lv_obj_t, x: i32, y: i32) -> &'static mut Self
```
- Creates static pixel frame arrays (const fn generated)
- Leaks `lv_img_dsc_t` + self via `Box::leak`
- Creates LVGL `lv_img_t` widget on the given screen
- Returns `&'static mut Self` for long-lived ownership without a global

### Pixel Art: Matrix Theme (32×32)

**Color palette (RGB565, LV_COLOR_16_SWAP=1):**
| Name         | RGB             | Usage                        |
|--------------|-----------------|------------------------------|
| BLACK        | #000000         | Background / void            |
| COAT_DK      | #0D0D0D (dark)  | Trench coat shadow           |
| COAT_MID     | #1A1A1A         | Trench coat midtone          |
| COAT_HL      | #2A2A2A         | Trench coat highlight        |
| MATRIX_GREEN | #00FF41         | Eyes, digital accents        |
| SKIN         | #C8845A         | Face/hands                   |
| HAIR_DK      | #0A0A0A         | Dark hair / shades           |

**Idle animation (2 frames):**
- Frame A: standing figure, long trench coat, green glowing eyes open, hands at sides
- Frame B: coat edge shifts 1px, eyes dim (half-closed blink)

**Walk animation (4 frames):**
- Frame A: left leg forward, right arm slightly forward
- Frame B: mid-stride, coat follows momentum (bottom pixels shift right)
- Frame C: right leg forward, left arm slightly forward
- Frame D: mid-stride, coat follows momentum (bottom pixels shift left)

**Sprite anatomy (32×32 grid):**
```
Row 0-3:   hair / top of head
Row 4-9:   face (ellipse), green eyes, thin nose
Row 10-13: neck + collar
Row 14-19: shoulders + upper coat
Row 20-27: torso + coat body
Row 28-31: legs / coat hem + feet
```

### Integration in `main.rs`

```rust
// Replace per-character globals with:
static mut MATRIX_CHAR: *mut MatrixCharacter = core::ptr::null_mut();

// In init:
MATRIX_CHAR = MatrixCharacter::new(SCREEN1, 200, 200);

// In the LVGL loop (replaces timer callbacks):
(*MATRIX_CHAR).update(5);

// To trigger movement (from any event handler):
(*MATRIX_CHAR).walk_to(100, 300);
```

The existing LVGL timer callbacks for crew/commander/blink are unchanged.

---

## Constraints

- All pixel data must be `'static` (LVGL holds raw pointers)
- Pixel arrays via `const fn` to keep them in flash, not RAM (matches existing pattern)
- No heap allocation at runtime (only `Box::leak` at startup)
- WALK_SPEED: 2 px per 5ms tick ≈ 400 px/s (crosses 400px screen in ~1s)
- Must not conflict with existing crew/cmd/blink timers

## Non-goals

- Collision detection
- Multiple simultaneous characters (can be added later by calling `MatrixCharacter::new` twice)
- Z-ordering (LVGL widget creation order determines z-order)
- Diagonal vs cardinal movement (Euclidean step is fine: move dx and dy each tick)
