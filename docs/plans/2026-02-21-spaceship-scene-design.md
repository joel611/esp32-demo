# Spaceship Bridge Scene — Design Doc

**Date:** 2026-02-21
**Status:** Approved
**Target:** Screen 1 (replaces Pikachu scene)

## Style Reference

`docs/design/style.png` — top-down pixel art spaceship bridge interior. Dark blue palette, star field visible through windows, 3 crew at stations + 1 commander at bottom center.

## Scene Layout (466×466)

```
┌────────────────────────────────────────────┐
│ [STAR WIN]  [BACK-CTR CONSOLE]  [STAR WIN] │
│                  [crew#3]                  │  ← crew member at top-center console
│  [L-CONSOLE]              [R-CONSOLE]      │
│    [crew#1]                  [crew#2]      │  ← crew at left/right consoles
│                                            │
│           FLOOR PANEL GRID                 │
│                                            │
│         [COMMANDER CONSOLE]                │
│              [commander]                   │
└────────────────────────────────────────────┘
```

## Color Palette (RGB565, LV_COLOR_16_SWAP=1)

| Name         | RGB hex | Role                          |
|--------------|---------|-------------------------------|
| `BG_DARK`    | #1a2040 | Room floor                    |
| `WALL`       | #2a3460 | Walls and structural elements |
| `CONSOLE`    | #1c2e3a | Console panel bodies          |
| `SCREEN_BLU` | #00c8ff | Monitor glow (cyan)           |
| `SCREEN_GRN` | #00ff88 | Readout panel (green)         |
| `WARN_RED`   | #ff2200 | Warning lights / blink state  |
| `METAL`      | #5060a0 | Metal trim / details          |
| `STAR_WHT`   | #ffffff | Stars in windows              |

All values must be byte-swapped at definition time for LV_COLOR_16_SWAP=1.

## Character Sprites

| Character    | Sprite size | Frames | Timer interval | Animation      |
|--------------|-------------|--------|----------------|----------------|
| Crew #1 (L)  | 32×64 px    | 2      | 500 ms         | idle sit / type |
| Crew #2 (R)  | 32×64 px    | 2      | 700 ms         | idle sit / type |
| Crew #3 (top)| 32×64 px    | 2      | 600 ms         | idle sit / head bob |
| Commander    | 40×72 px    | 3      | 800 ms         | stand / point-L / point-R |
| Console blink| 20×10 px   | 2      | 1200 ms        | glow color swap |

Crew sprites share the same body shape — dark uniform, light skin/visor, pixel art consistent with reference style.

## Implementation Approach: Layered Sprites

- **Background:** One static 466×466 `const fn` computed pixel array stored in flash (~424KB).
- **Characters:** Four small sprite arrays (+ console blink), each positioned as an LVGL `lv_img` widget layered over the background.
- **Animation:** Three independent `lv_timer` callbacks cycling crew, commander, and console blink at different rates.

## New File: `src/spaceship.rs`

Functions:
- `pixel_bg(x, y) -> u16` — full 466×466 background geometry
- `pixel_crew_1_a/b(x, y) -> u16` — crew #1 idle frames
- `pixel_crew_2_a/b(x, y) -> u16` — crew #2 idle frames
- `pixel_crew_3_a/b(x, y) -> u16` — crew #3 idle frames
- `pixel_commander_a/b/c(x, y) -> u16` — commander 3-frame animation
- `pixel_console_a/b(x, y) -> u16` — console blink small sprite

Static arrays (all `'static`, `pub`):
- `BG_FRAME: [u16; 217156]`
- `CREW1_A/B: [u16; 2048]` (32×64)
- `CREW2_A/B: [u16; 2048]`
- `CREW3_A/B: [u16; 2048]`
- `CMD_A/B/C: [u16; 2880]` (40×72)
- `CONSOLE_A/B: [u16; 200]` (20×10)

## Changes to `src/main.rs`

Screen 1 init:
1. Remove all Pikachu code
2. Add `mod spaceship;`
3. Create background `lv_img` widget (full 466×466)
4. Create 4 character + 1 console `lv_img` widgets at their station (x, y) positions
5. Register 3 timers: `crew_timer_cb`, `cmd_timer_cb`, `console_timer_cb`

Global statics to add:
- `CREW_IMG: [*mut lv_obj_t; 3]`
- `CMD_IMG: *mut lv_obj_t`
- `CONSOLE_IMG: *mut lv_obj_t`
- Frame index per animated element

## Flash Usage Estimate

| Asset            | Size     |
|------------------|----------|
| Background       | ~424 KB  |
| 8 crew frames    | ~32 KB   |
| 3 commander frames | ~17 KB |
| Console blink    | < 1 KB   |
| **Total**        | **~473 KB** |

Well within ESP32-S3 flash capacity (typically 4–8 MB).

## Character Position Map (approximate, from top-left)

| Character  | Center X | Center Y |
|------------|----------|----------|
| Crew #3    | 233      | 110      |
| Crew #1    | 110      | 240      |
| Crew #2    | 356      | 240      |
| Commander  | 233      | 380      |
| Console blink | 233   | 60       |
