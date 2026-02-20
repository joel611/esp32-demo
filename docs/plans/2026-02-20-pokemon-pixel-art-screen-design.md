# Pokemon-like Pixel Art Screen with Idle Animation

**Date:** 2026-02-20
**Status:** Approved

## Summary

Replace Screen 1 on the ESP32-S3 AMOLED display with a Pokemon-style pixel art scene featuring a 64×64 sprite character with a 2-frame idle animation (frame swap every 600ms).

## Display Context

- Hardware: 466×466 circular AMOLED (SH8601/CO5300 QSPI)
- Framework: LVGL 8.x via `lvgl-sys` Rust FFI
- Existing: Screen 2 (blue bg), swipe gesture navigation between screens

## Screen Layout

```
┌─────────────────────────────┐
│                             │
│                             │
│          ┌──────┐           │
│          │      │           │
│          │SPRITE│  64×64    │
│          │      │           │
│          └──────┘           │
│         NAME / HP           │
│                             │
│  [swipe left → Screen 2]    │
└─────────────────────────────┘
```

- Black background (`0x00, 0x00, 0x00`)
- Centered `lv_img` widget (64×64 px)
- Name label below sprite in yellow (e.g. "PIKACHU")
- Swipe left → Screen 2 (unchanged)

## Sprite Data

- Two RGB565 frame arrays: 64×64 = 4,096 `u16` values each
- Frame A: default pose
- Frame B: idle variation (ear twitch or eye blink)
- Defined as Rust `const` arrays in `src/sprites.rs`
- Wrapped in `lv_img_dsc_t` with `'static` lifetime (required by LVGL 8.x)

## Animation Mechanism

- `lv_timer_create` fires every 600ms
- Callback toggles frame index (0 → 1 → 0)
- Uses `lv_img_set_src` to swap between two `lv_img_dsc_t` descriptors
- No C changes; existing `lv_tick_inc` / `lv_timer_handler` loop drives it

## Files Changed

| File | Change |
|------|--------|
| `src/main.rs` | Replace Screen 1 body: black bg + `lv_img` + name label + `lv_timer` |
| `src/sprites.rs` | New: two RGB565 64×64 frame arrays + `lv_img_dsc_t` descriptors |

## Constraints

- Static lifetime: `lv_img_dsc_t` and pixel data must outlive LVGL display registration
- RGB565 pixel format with `LV_COLOR_16_SWAP 1` (byte-swapped for ESP32 little-endian)
- Gesture/swipe callbacks unchanged

## Out of Scope

- No background art (pure black)
- No sound
- No interaction with the sprite (touch/tap not handled)
- No health bar or battle UI
