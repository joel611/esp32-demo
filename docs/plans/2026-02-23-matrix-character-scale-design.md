# MatrixCharacter Scale (Density) Setting — Design

**Date:** 2026-02-23

## Goal

Add a runtime `scale: u16` parameter to `MatrixCharacter::new()` that enlarges the sprite via LVGL zoom while preserving the pixel art (nearest-neighbor) look.

## Approach: LVGL `lv_img_set_zoom`

Chosen over compile-time scaled buffers (extra flash) and runtime CPU scaling (heap complexity).

- `lv_img_set_zoom(widget, 256 * scale)` — 256 = 100%, 512 = 200%, 768 = 300%
- `lv_img_set_pivot(widget, 0, 0)` — anchor zoom at top-left so `lv_obj_set_pos` coordinates are intuitive
- `LV_IMG_ANTIALIAS` is absent from `lv_conf.h` (defaults to 0 = nearest-neighbor) — no config change needed
- Sprite data remains 32×32 in flash; zero extra flash/RAM cost

## Coordinate Space

All coordinates (`walk_to`, `lv_obj_set_pos`) are in **unscaled pixel space**. LVGL expands the rendered sprite outward from the top-left anchor. At `scale=3`, the on-screen footprint is 96×96 px; callers simply pick `(x, y)` as the desired top-left of that rendered area.

## API

```rust
// Before
pub unsafe fn new(screen: *mut lv_obj_t, x: i32, y: i32) -> &'static mut Self

// After
pub unsafe fn new(screen: *mut lv_obj_t, x: i32, y: i32, scale: u16) -> &'static mut Self
```

`scale` is stored in the struct for future use (e.g., hitbox queries). Minimum valid value is 1.

## Wire-up

```rust
MATRIX_CHAR = MatrixCharacter::new(SCREEN1, 200, 200, 3);
// Renders as 96×96 px at (200, 200) on the 466×466 display
```

## Files Changed

- `src/matrix_character.rs` — add `scale: u16` field, set zoom+pivot in `new()`
- `src/main.rs` — pass `scale=3` (or desired value) to `new()`
