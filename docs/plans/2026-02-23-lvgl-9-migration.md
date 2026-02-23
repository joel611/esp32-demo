# LVGL 9.5 Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade from `lvgl-sys 0.6.2` (LVGL 8.3.x) to `lightvgl-sys 9.5.0` (LVGL 9.5) and update all calling code.

**Architecture:** Swap the raw bindings crate (`lvgl-sys` → `lightvgl-sys`), replace `lv_conf.h` with LVGL 9.x format, then mechanically update every renamed API across `main.rs` and `matrix_character.rs`. The safe `lvgl` wrapper crate is dropped entirely — the code uses raw bindings directly. No behavioral changes; this is a pure API migration.

**Tech Stack:** Rust + `lightvgl-sys 9.5.0`, ESP32-S3, ESP-IDF v5.3.3, xtensa-esp32s3-espidf target.

---

## Key API Changes Reference (LVGL 8 → 9)

| LVGL 8 | LVGL 9 |
|--------|--------|
| `lv_disp_drv_t` + `lv_disp_draw_buf_t` structs | removed — use `lv_display_create()` |
| `lv_disp_drv_init()` / `lv_disp_drv_register()` | `lv_display_create(w, h)` |
| `lv_disp_draw_buf_init()` | `lv_display_set_buffers()` |
| `disp_drv.flush_cb = Some(cb)` | `lv_display_set_flush_cb(disp, Some(cb))` |
| `lv_disp_flush_ready(drv)` | `lv_display_flush_ready(disp)` |
| `lv_disp_get_default()` | `lv_display_get_default()` |
| `lv_disp_get_scr_act(disp)` | `lv_screen_active()` |
| flush cb: `(disp_drv: *mut lv_disp_drv_t, area, color_p: *mut lv_color_t)` | `(disp: *mut lv_display_t, area, px_map: *mut u8)` |
| `lv_indev_drv_t` struct | removed — use `lv_indev_create()` |
| `lv_indev_drv_init()` / `lv_indev_drv_register()` | `lv_indev_create()` |
| `indev_drv.type_` / `indev_drv.read_cb` | `lv_indev_set_type()` / `lv_indev_set_read_cb()` |
| touch cb: `(_drv: *mut lv_indev_drv_t, data)` | `(_indev: *mut lv_indev_t, data)` |
| `lv_indev_get_act()` | `lv_indev_active()` |
| `lv_img_create()` | `lv_image_create()` |
| `lv_img_set_src()` | `lv_image_set_src()` |
| `lv_img_set_zoom(w, factor_256)` | `lv_image_set_scale(w, factor_256)` |
| `lv_img_set_pivot()` | `lv_image_set_pivot()` |
| `lv_img_dsc_t` | `lv_image_dsc_t` |
| `lv_img_dsc_t.header.set_cf(LV_IMG_CF_TRUE_COLOR)` | `lv_image_dsc_t.header.cf = LV_COLOR_FORMAT_RGB565_SWAP` |
| `lv_img_dsc_t.header.set_w(w)` / `.set_h(h)` | `.header.w = w` / `.header.h = h` |
| (no stride field) | `.header.stride = w * 2` (bytes per row for RGB565) |
| `lv_scr_load_anim()` | `lv_screen_load_anim()` |
| `_LV_COLOR_MAKE(r, g, b)` | `lv_color_make(r, g, b)` |
| `lv_coord_t` (i16) | `i32` (no lv_coord_t in v9) |
| `LV_COLOR_16_SWAP 1` in lv_conf.h | removed — set on display: `lv_display_set_color_format(disp, LV_COLOR_FORMAT_RGB565_SWAP)` |
| `LV_IMG_CF_TRUE_COLOR` constant | `lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAP` |
| `lv_indev_state_t_LV_INDEV_STATE_PRESSED` | same name in v9 |

> **Note on Rust crate name:** The crate is added as `lightvgl-sys` in Cargo.toml but referenced as `lightvgl_sys::` in Rust code (hyphen → underscore). Every `lvgl_sys::` reference becomes `lightvgl_sys::`.

---

## Task 1: Swap Cargo.toml dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Edit Cargo.toml**

Replace the `[dependencies]` block's LVGL entries:

```toml
# BEFORE:
lvgl = { version = "0.6.2", default-features= false, features= [
  "embedded_graphics",
  "unsafe_no_autoinit",
] }
lvgl-sys = "0.6.2"

# AFTER:
lightvgl-sys = "9.5.0"
```

Remove both the `lvgl` and `lvgl-sys` lines entirely and add `lightvgl-sys = "9.5.0"`.

**Step 2: Attempt a build to see all errors**

```bash
cargo build 2>&1 | head -100
```

Expected: Many compile errors. The goal here is just to confirm the crate resolves and the build system finds the dep. The errors will guide Tasks 2–7.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: swap lvgl-sys 0.6.2 for lightvgl-sys 9.5.0"
```

---

## Task 2: Replace lv_conf.h with LVGL 9.x version

**Files:**
- Modify: `lvgl-configs/lv_conf.h`

**Context:** The existing `lv_conf.h` is for LVGL 8.3.0-dev. Many options were removed or renamed in v9. Replace the entire file with a minimal v9-compatible config that preserves the settings we need.

Key settings to preserve/update:
- `LV_COLOR_DEPTH 16` — keep (RGB565)
- `LV_COLOR_16_SWAP` — **remove** (gone in v9; set on display driver instead)
- `LV_MEM_SIZE` — keep at `(48U * 1024U)` or increase to `(64U * 1024U)` for v9
- `LV_FONT_MONTSERRAT_14 1` and `LV_FONT_MONTSERRAT_28 1` — keep
- `LV_FONT_DEFAULT &lv_font_montserrat_28` — keep
- `LV_USE_IMG 1` → `LV_USE_IMAGE 1` (widget renamed in v9)
- `LV_USE_PERF_MONITOR`, `LV_USE_MEM_MONITOR` — keep
- Remove all GPU sections (STM32, NXP, SDL — replaced with v9 draw system)
- Remove `LV_DRAW_COMPLEX` (replaced with `LV_USE_DRAW_SW`)
- Remove `LV_USE_LARGE_COORD` (int32_t is default in v9)
- Remove `LV_DISP_ROT_MAX_BUF`, `LV_DISP_DEF_REFR_PERIOD` → `LV_DEF_REFR_PERIOD`

**Step 1: Replace the file content**

Write the new `lvgl-configs/lv_conf.h`:

```c
/**
 * @file lv_conf.h
 * Configuration file for LVGL v9.x
 * Targeting: ESP32-S3 QSPI AMOLED 466x466, RGB565 with byte-swap
 */

/* clang-format off */
#if 1 /*Set it to "1" to enable content*/

#ifndef LV_CONF_H
#define LV_CONF_H

#include <stdint.h>

/*====================
   COLOR SETTINGS
 *====================*/

/* Color depth: 16 = RGB565. Byte-swap (LV_COLOR_16_SWAP) is removed in v9;
 * call lv_display_set_color_format(disp, LV_COLOR_FORMAT_RGB565_SWAP) in driver init instead. */
#define LV_COLOR_DEPTH 16

/* Chroma key color (not drawn if used as transparent) */
#define LV_COLOR_CHROMA_KEY lv_color_hex(0x00ff00)

/*=========================
   MEMORY SETTINGS
 *=========================*/

/* 0: use LVGL's built-in allocator */
#define LV_MEM_CUSTOM 0
#if LV_MEM_CUSTOM == 0
    #define LV_MEM_SIZE (64U * 1024U)   /* bytes; 64KB for v9 */
    #define LV_MEM_ADR  0               /* 0 = use internal array */
#else
    #define LV_MEM_CUSTOM_INCLUDE <stdlib.h>
    #define LV_MEM_CUSTOM_ALLOC   malloc
    #define LV_MEM_CUSTOM_FREE    free
    #define LV_MEM_CUSTOM_REALLOC realloc
#endif

#define LV_MEM_BUF_MAX_NUM 16

/*====================
   HAL SETTINGS
 *====================*/

/* Display refresh period [ms] */
#define LV_DEF_REFR_PERIOD 10

/* Input device read period [ms] */
#define LV_INDEV_DEF_READ_PERIOD 30

/* Manual tick via lv_tick_inc() */
#define LV_TICK_CUSTOM 0

#define LV_DPI_DEF 130

/*=======================
 * RENDERING
 *=======================*/

/* Software renderer — required for ESP32-S3 (no GPU) */
#define LV_USE_DRAW_SW 1
#if LV_USE_DRAW_SW
    #define LV_DRAW_SW_COMPLEX 1
    #define LV_DRAW_SW_SUPPORT_RGB565 1
    #define LV_DRAW_SW_SUPPORT_RGB888 0
    #define LV_DRAW_SW_SUPPORT_ARGB8888 0
    #define LV_DRAW_SW_SUPPORT_XRGB8888 0
    #define LV_DRAW_SW_SUPPORT_A8 0
    #define LV_DRAW_SW_SHADOW_CACHE_SIZE 0
    #define LV_DRAW_SW_CIRCLE_CACHE_CNT 4
#endif

#define LV_DRAW_LAYER_SIMPLE_BUF_SIZE (24 * 1024)

/*-------------
 * Logging
 *-----------*/
#define LV_USE_LOG 0

/*-------------
 * Asserts
 *-----------*/
#define LV_USE_ASSERT_NULL          1
#define LV_USE_ASSERT_MALLOC        1
#define LV_USE_ASSERT_STYLE         0
#define LV_USE_ASSERT_MEM_INTEGRITY 0
#define LV_USE_ASSERT_OBJ           0

#define LV_ASSERT_HANDLER_INCLUDE <stdint.h>
#define LV_ASSERT_HANDLER while(1);

/*-------------
 * Performance monitoring
 *-----------*/
#define LV_USE_PERF_MONITOR 1
#if LV_USE_PERF_MONITOR
    #define LV_USE_PERF_MONITOR_POS LV_ALIGN_BOTTOM_RIGHT
#endif

#define LV_USE_MEM_MONITOR 1
#if LV_USE_MEM_MONITOR
    #define LV_USE_MEM_MONITOR_POS LV_ALIGN_BOTTOM_LEFT
#endif

/*=====================
 *  COMPILER SETTINGS
 *====================*/

#define LV_BIG_ENDIAN_SYSTEM        0
#define LV_ATTRIBUTE_TICK_INC
#define LV_ATTRIBUTE_TIMER_HANDLER
#define LV_ATTRIBUTE_FLUSH_READY
#define LV_ATTRIBUTE_MEM_ALIGN_SIZE 1
#define LV_ATTRIBUTE_MEM_ALIGN
#define LV_ATTRIBUTE_LARGE_CONST
#define LV_ATTRIBUTE_LARGE_RAM_ARRAY
#define LV_ATTRIBUTE_FAST_MEM
#define LV_ATTRIBUTE_DMA
#define LV_EXPORT_CONST_INT(int_value) struct _silence_gcc_warning

#define LV_USE_USER_DATA 1
#define LV_ENABLE_GC     0

/*==================
 *   FONT USAGE
 *===================*/

#define LV_FONT_MONTSERRAT_8  0
#define LV_FONT_MONTSERRAT_10 0
#define LV_FONT_MONTSERRAT_12 0
#define LV_FONT_MONTSERRAT_14 1
#define LV_FONT_MONTSERRAT_16 0
#define LV_FONT_MONTSERRAT_18 0
#define LV_FONT_MONTSERRAT_20 0
#define LV_FONT_MONTSERRAT_22 0
#define LV_FONT_MONTSERRAT_24 0
#define LV_FONT_MONTSERRAT_26 0
#define LV_FONT_MONTSERRAT_28 1
#define LV_FONT_MONTSERRAT_30 0
#define LV_FONT_MONTSERRAT_32 0
#define LV_FONT_MONTSERRAT_34 0
#define LV_FONT_MONTSERRAT_36 0
#define LV_FONT_MONTSERRAT_38 0
#define LV_FONT_MONTSERRAT_40 0
#define LV_FONT_MONTSERRAT_42 0
#define LV_FONT_MONTSERRAT_44 0
#define LV_FONT_MONTSERRAT_46 0
#define LV_FONT_MONTSERRAT_48 0

#define LV_FONT_MONTSERRAT_12_SUBPX      0
#define LV_FONT_MONTSERRAT_28_COMPRESSED 0
#define LV_FONT_DEJAVU_16_PERSIAN_HEBREW 0
#define LV_FONT_SIMSUN_16_CJK            0
#define LV_FONT_UNSCII_8  0
#define LV_FONT_UNSCII_16 0
#define LV_FONT_CUSTOM_DECLARE

#define LV_FONT_DEFAULT &lv_font_montserrat_28

#define LV_FONT_FMT_TXT_LARGE  0
#define LV_USE_FONT_COMPRESSED 0
#define LV_USE_FONT_SUBPX      0

/*=================
 *  TEXT SETTINGS
 *=================*/

#define LV_TXT_ENC LV_TXT_ENC_UTF8
#define LV_TXT_BREAK_CHARS " ,.;:-_"
#define LV_TXT_LINE_BREAK_LONG_LEN          0
#define LV_TXT_LINE_BREAK_LONG_PRE_MIN_LEN  3
#define LV_TXT_LINE_BREAK_LONG_POST_MIN_LEN 3
#define LV_TXT_COLOR_CMD "#"
#define LV_USE_BIDI 0
#define LV_USE_ARABIC_PERSIAN_CHARS 0

/*==================
 *  WIDGET USAGE
 *================*/

#define LV_USE_ARC        1
#define LV_USE_BAR        1
#define LV_USE_BTN        1
#define LV_USE_BTNMATRIX  1
#define LV_USE_CANVAS     0
#define LV_USE_CHECKBOX   0
#define LV_USE_DROPDOWN   0
#define LV_USE_IMAGE      1  /* NOTE: was LV_USE_IMG in v8 */
#define LV_USE_LABEL      1
#if LV_USE_LABEL
    #define LV_LABEL_TEXT_SELECTION  1
    #define LV_LABEL_LONG_TXT_HINT   1
#endif
#define LV_USE_LINE       1
#define LV_USE_ROLLER     0
#define LV_USE_SLIDER     0
#define LV_USE_SWITCH     0
#define LV_USE_TEXTAREA   0
#define LV_USE_TABLE      0

/*==================
 * EXTRA COMPONENTS
 *==================*/

#define LV_USE_ANIMIMG    0
#define LV_USE_CALENDAR   0
#define LV_USE_CHART      0
#define LV_USE_COLORWHEEL 0
#define LV_USE_IMGBTN     0
#define LV_USE_KEYBOARD   0
#define LV_USE_LED        0
#define LV_USE_LIST       0
#define LV_USE_MENU       0
#define LV_USE_METER      0
#define LV_USE_MSGBOX     0
#define LV_USE_SPINBOX    0
#define LV_USE_SPINNER    0
#define LV_USE_TABVIEW    0
#define LV_USE_TILEVIEW   1
#define LV_USE_WIN        0
#define LV_USE_SPAN       0

/*-----------
 * Themes
 *----------*/
#define LV_USE_THEME_DEFAULT 1
#if LV_USE_THEME_DEFAULT
    #define LV_THEME_DEFAULT_DARK           1
    #define LV_THEME_DEFAULT_GROW           0
    #define LV_THEME_DEFAULT_TRANSITION_TIME 80
#endif
#define LV_USE_THEME_SIMPLE 1
#define LV_USE_THEME_MONO   0

/*-----------
 * Layouts
 *----------*/
#define LV_USE_FLEX 1
#define LV_USE_GRID 0

/*---------------------
 * 3rd party libraries
 *--------------------*/
#define LV_USE_FS_STDIO  0
#define LV_USE_FS_POSIX  0
#define LV_USE_FS_WIN32  0
#define LV_USE_FS_FATFS  0
#define LV_USE_PNG       0
#define LV_USE_BMP       0
#define LV_USE_SJPG      0
#define LV_USE_GIF       0
#define LV_USE_QRCODE    0
#define LV_USE_FREETYPE  0
#define LV_USE_RLOTTIE   0
#define LV_USE_FFMPEG    0

/*-----------
 * Others
 *----------*/
#define LV_USE_SNAPSHOT 0
#define LV_USE_MONKEY   0
#define LV_USE_GRIDNAV  0
#define LV_USE_FRAGMENT 0

#define LV_BUILD_EXAMPLES 0

/*--END OF LV_CONF_H--*/
#endif /*LV_CONF_H*/
#endif /*End of "Content enable"*/
```

**Step 2: Build to check the config is accepted**

```bash
cargo build 2>&1 | grep -i "lv_conf\|conf error\|unknown" | head -30
```

Expected: No lv_conf.h-specific errors. Rust compile errors are expected and handled in later tasks.

**Step 3: Commit**

```bash
git add lvgl-configs/lv_conf.h
git commit -m "chore: rewrite lv_conf.h for LVGL 9.x (remove LV_COLOR_16_SWAP, update draw system)"
```

---

## Task 3: Update display driver init in main.rs

**Files:**
- Modify: `src/main.rs`

**Context:** LVGL 9 replaced the `lv_disp_drv_t` + `lv_disp_draw_buf_t` driver structs with a simpler functional API. Byte-swap (formerly `LV_COLOR_16_SWAP=1`) is now set via `lv_display_set_color_format()`.

**Step 1: Update the flush callback signature (main.rs:70-84)**

Old:
```rust
unsafe extern "C" fn lvgl_flush_cb(
    disp_drv: *mut lvgl_sys::lv_disp_drv_t,
    area: *const lvgl_sys::lv_area_t,
    color_p: *mut lvgl_sys::lv_color_t,
) {
    lcd_wait_flush_done();
    let x1 = (*area).x1 as i32;
    let y1 = (*area).y1 as i32;
    let x2 = (*area).x2 as i32 + 1;
    let y2 = (*area).y2 as i32 + 1;
    lcd_draw_bitmap_async(x1, y1, x2, y2, color_p as *const _);
    lvgl_sys::lv_disp_flush_ready(disp_drv);
}
```

New:
```rust
unsafe extern "C" fn lvgl_flush_cb(
    disp: *mut lightvgl_sys::lv_display_t,
    area: *const lightvgl_sys::lv_area_t,
    px_map: *mut u8,
) {
    lcd_wait_flush_done();
    let x1 = (*area).x1 as i32;
    let y1 = (*area).y1 as i32;
    let x2 = (*area).x2 as i32 + 1;
    let y2 = (*area).y2 as i32 + 1;
    lcd_draw_bitmap_async(x1, y1, x2, y2, px_map as *const _);
    lightvgl_sys::lv_display_flush_ready(disp);
}
```

**Step 2: Update display init block (main.rs:197-235)**

Replace the entire block from `// ── 2. LVGL init` through `log::info!("LVGL display registered")`:

```rust
        // ── 2. LVGL init ──────────────────────────────────────────────────────
        lightvgl_sys::lv_init();

        // ── 3. Two DMA-capable pixel buffers for double-buffering ─────────────
        // Must be internal SRAM: esp-lcd SPI driver calls esp_ptr_dma_capable()
        // which rejects PSRAM. Two 100-row buffers (~182KB total).
        let pixel_size = core::mem::size_of::<lightvgl_sys::lv_color_t>();
        let buf1 = esp_idf_svc::sys::heap_caps_malloc(
            DRAW_BUF_PIXELS * pixel_size,
            esp_idf_svc::sys::MALLOC_CAP_DMA,
        ) as *mut core::ffi::c_void;
        let buf2 = esp_idf_svc::sys::heap_caps_malloc(
            DRAW_BUF_PIXELS * pixel_size,
            esp_idf_svc::sys::MALLOC_CAP_DMA,
        ) as *mut core::ffi::c_void;
        assert!(!buf1.is_null() && !buf2.is_null(), "LVGL draw buf alloc failed");

        // ── 4. Display (LVGL 9 API) ───────────────────────────────────────────
        let disp = lightvgl_sys::lv_display_create(LCD_W as i32, LCD_H as i32);
        assert!(!disp.is_null(), "lv_display_create failed");

        // Byte-swap RGB565 for SPI: replaces LV_COLOR_16_SWAP=1 from lv_conf.h
        lightvgl_sys::lv_display_set_color_format(
            disp,
            lightvgl_sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAP,
        );
        lightvgl_sys::lv_display_set_flush_cb(disp, Some(lvgl_flush_cb));
        lightvgl_sys::lv_display_set_buffers(
            disp,
            buf1,
            buf2,
            (DRAW_BUF_PIXELS * pixel_size) as u32,
            lightvgl_sys::lv_display_render_mode_t_LV_DISPLAY_RENDER_MODE_PARTIAL,
        );
        log::info!("LVGL display registered");
```

**Step 3: Build**

```bash
cargo build 2>&1 | grep "^error" | head -40
```

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: update LVGL display driver to v9 API (lv_display_create + set_buffers)"
```

---

## Task 4: Update input device init and touch callback in main.rs

**Files:**
- Modify: `src/main.rs`

**Context:** LVGL 9 replaced `lv_indev_drv_t` with a functional API. No need to Box::leak the driver struct — `lv_indev_create()` returns a managed pointer.

**Step 1: Update touch callback signature (main.rs:88-99)**

Old:
```rust
unsafe extern "C" fn lvgl_touch_cb(
    _drv: *mut lvgl_sys::lv_indev_drv_t,
    data: *mut lvgl_sys::lv_indev_data_t,
) {
    if TOUCH_PRESSED.load(Ordering::Relaxed) {
        (*data).point.x = TOUCH_X.load(Ordering::Relaxed) as lvgl_sys::lv_coord_t;
        (*data).point.y = TOUCH_Y.load(Ordering::Relaxed) as lvgl_sys::lv_coord_t;
        (*data).state = lvgl_sys::lv_indev_state_t_LV_INDEV_STATE_PRESSED;
    } else {
        (*data).state = lvgl_sys::lv_indev_state_t_LV_INDEV_STATE_RELEASED;
    }
}
```

New (note: `lv_coord_t` removed in v9 — coordinates are `i32`):
```rust
unsafe extern "C" fn lvgl_touch_cb(
    _indev: *mut lightvgl_sys::lv_indev_t,
    data: *mut lightvgl_sys::lv_indev_data_t,
) {
    if TOUCH_PRESSED.load(Ordering::Relaxed) {
        (*data).point.x = TOUCH_X.load(Ordering::Relaxed);
        (*data).point.y = TOUCH_Y.load(Ordering::Relaxed);
        (*data).state = lightvgl_sys::lv_indev_state_t_LV_INDEV_STATE_PRESSED;
    } else {
        (*data).state = lightvgl_sys::lv_indev_state_t_LV_INDEV_STATE_RELEASED;
    }
}
```

**Step 2: Update indev init block (main.rs:237-244)**

Old:
```rust
        // ── 6. Input device (touch) ───────────────────────────────────────────
        let indev_drv: &'static mut lvgl_sys::lv_indev_drv_t =
            Box::leak(Box::new(core::mem::zeroed()));
        lvgl_sys::lv_indev_drv_init(indev_drv);
        indev_drv.type_ = lvgl_sys::lv_indev_type_t_LV_INDEV_TYPE_POINTER;
        indev_drv.read_cb = Some(lvgl_touch_cb);
        lvgl_sys::lv_indev_drv_register(indev_drv);
        log::info!("LVGL touch input registered");
```

New:
```rust
        // ── 6. Input device (touch) ───────────────────────────────────────────
        let indev = lightvgl_sys::lv_indev_create();
        assert!(!indev.is_null(), "lv_indev_create failed");
        lightvgl_sys::lv_indev_set_type(
            indev,
            lightvgl_sys::lv_indev_type_t_LV_INDEV_TYPE_POINTER,
        );
        lightvgl_sys::lv_indev_set_read_cb(indev, Some(lvgl_touch_cb));
        log::info!("LVGL touch input registered");
```

**Step 3: Build**

```bash
cargo build 2>&1 | grep "^error" | head -40
```

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: update LVGL input device to v9 API (lv_indev_create)"
```

---

## Task 5: Update screen, color, and gesture API in main.rs

**Files:**
- Modify: `src/main.rs`

### 5a: Screen query (main.rs:248)

Old:
```rust
SCREEN1 = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());
```
New:
```rust
SCREEN1 = lightvgl_sys::lv_screen_active();
```

### 5b: Background color (main.rs:253-257)

Old:
```rust
        lvgl_sys::lv_obj_set_style_bg_color(
            SCREEN1,
            lvgl_sys::_LV_COLOR_MAKE(0x1a, 0x20, 0x40),
            lvgl_sys::LV_STATE_DEFAULT,
        );
```
New:
```rust
        lightvgl_sys::lv_obj_set_style_bg_color(
            SCREEN1,
            lightvgl_sys::lv_color_make(0x1a, 0x20, 0x40),
            lightvgl_sys::LV_STATE_DEFAULT,
        );
```

### 5c: Screen 2 background color (main.rs:349-353)

Old:
```rust
        lvgl_sys::lv_obj_set_style_bg_color(
            SCREEN2,
            lvgl_sys::_LV_COLOR_MAKE(0x00, 0x30, 0x80),
            lvgl_sys::LV_STATE_DEFAULT,
        );
```
New:
```rust
        lightvgl_sys::lv_obj_set_style_bg_color(
            SCREEN2,
            lightvgl_sys::lv_color_make(0x00, 0x30, 0x80),
            lightvgl_sys::LV_STATE_DEFAULT,
        );
```

### 5d: Gesture callback (main.rs:104-132)

Old:
```rust
unsafe extern "C" fn gesture_cb(e: *mut lvgl_sys::lv_event_t) {
    let indev = lvgl_sys::lv_indev_get_act();
    ...
    let active = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());

    if dir == lvgl_sys::LV_DIR_LEFT as lvgl_sys::lv_dir_t && active == SCREEN1 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN2,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_LEFT,
            150, 0, false,
        );
    } else if dir == lvgl_sys::LV_DIR_RIGHT as lvgl_sys::lv_dir_t && active == SCREEN2 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN1,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_RIGHT,
            150, 0, false,
        );
    }
    let _ = e;
}
```
New:
```rust
unsafe extern "C" fn gesture_cb(e: *mut lightvgl_sys::lv_event_t) {
    let indev = lightvgl_sys::lv_indev_active();
    if indev.is_null() {
        return;
    }
    let dir = lightvgl_sys::lv_indev_get_gesture_dir(indev);
    let active = lightvgl_sys::lv_screen_active();

    if dir == lightvgl_sys::LV_DIR_LEFT as lightvgl_sys::lv_dir_t && active == SCREEN1 {
        lightvgl_sys::lv_screen_load_anim(
            SCREEN2,
            lightvgl_sys::lv_screen_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_LEFT,
            150, 0, false,
        );
    } else if dir == lightvgl_sys::LV_DIR_RIGHT as lightvgl_sys::lv_dir_t && active == SCREEN2 {
        lightvgl_sys::lv_screen_load_anim(
            SCREEN1,
            lightvgl_sys::lv_screen_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_RIGHT,
            150, 0, false,
        );
    }
    let _ = e;
}
```

### 5e: Event callback registration (main.rs:360-371)

Replace all `lvgl_sys::` with `lightvgl_sys::` in the `lv_obj_add_event_cb` calls:
```rust
        lightvgl_sys::lv_obj_add_event_cb(
            SCREEN1,
            Some(gesture_cb),
            lightvgl_sys::lv_event_code_t_LV_EVENT_GESTURE,
            core::ptr::null_mut(),
        );
        lightvgl_sys::lv_obj_add_event_cb(
            SCREEN2,
            Some(gesture_cb),
            lightvgl_sys::lv_event_code_t_LV_EVENT_GESTURE,
            core::ptr::null_mut(),
        );
```

**Step 1: Apply all 5a–5e changes**

**Step 2: Build**

```bash
cargo build 2>&1 | grep "^error" | head -40
```

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: update screen, color, and gesture API to LVGL v9"
```

---

## Task 6: Update image widget and descriptors in main.rs

**Files:**
- Modify: `src/main.rs`

**Context:** In LVGL 9, all `lv_img_*` functions are renamed to `lv_image_*`. The `lv_img_dsc_t` struct becomes `lv_image_dsc_t` with a new header format that includes a `stride` field (bytes per row).

### 6a: Static pointer types (main.rs:55-62)

Old:
```rust
static mut CREW_DSC_A: *const lvgl_sys::lv_img_dsc_t = core::ptr::null();
// ... (7 similar lines)
```
New:
```rust
static mut CREW_DSC_A: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CREW_DSC_B: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_A:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_B:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_C:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BLINK_DSC_A: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BLINK_DSC_B: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BG_DSC: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
```

### 6b: `make_dsc` function (main.rs:136-144)

Old:
```rust
fn make_dsc(pixels: &'static [u16], w: u32, h: u32) -> lvgl_sys::lv_img_dsc_t {
    let mut dsc = lvgl_sys::lv_img_dsc_t::default();
    dsc.header.set_cf(lvgl_sys::LV_IMG_CF_TRUE_COLOR as u32);
    dsc.header.set_w(w);
    dsc.header.set_h(h);
    dsc.data_size = (w * h * core::mem::size_of::<u16>() as u32) as u32;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}
```
New:
```rust
fn make_dsc(pixels: &'static [u16], w: u32, h: u32) -> lightvgl_sys::lv_image_dsc_t {
    let mut dsc = lightvgl_sys::lv_image_dsc_t::default();
    // LV_COLOR_FORMAT_RGB565_SWAP: our pixels are byte-swapped RGB565
    dsc.header.cf = lightvgl_sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAP as u32;
    dsc.header.w = w;
    dsc.header.h = h;
    dsc.header.stride = w * 2;  // bytes per row for RGB565
    dsc.data_size = w * h * 2;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}
```

> **Build note:** The exact field names on `lv_image_dsc_t.header` (cf, w, h, stride) may still use bitfield setters in the generated bindings. If you get errors like "no field `cf`", check the bindings: `grep "lv_image_header\|set_cf\|set_w\|set_h\|stride" target/xtensa-esp32s3-espidf/debug/build/lightvgl-sys-*/out/bindings.rs`. Use the generated setter names if direct field assignment doesn't compile.

### 6c: Timer callbacks (main.rs:148-172)

In `crew_timer_cb`, `cmd_timer_cb`, `blink_timer_cb` — change the callback parameter type and `lv_img_set_src` call:

Old:
```rust
unsafe extern "C" fn crew_timer_cb(_timer: *mut lvgl_sys::lv_timer_t) {
    ...
    lvgl_sys::lv_img_set_src(CREW_WIDGETS[i], src as *const _);
```
New:
```rust
unsafe extern "C" fn crew_timer_cb(_timer: *mut lightvgl_sys::lv_timer_t) {
    ...
    lightvgl_sys::lv_image_set_src(CREW_WIDGETS[i], src as *const _);
```

Apply same rename to `cmd_timer_cb` and `blink_timer_cb`.

### 6d: `lv_img_create` calls (main.rs:266, 295, 310, 326)

Replace all `lvgl_sys::lv_img_create(parent)` → `lightvgl_sys::lv_image_create(parent)`.

### 6e: `lv_img_set_src` calls (main.rs:267, 296, 311, 327)

Replace all `lvgl_sys::lv_img_set_src(...)` → `lightvgl_sys::lv_image_set_src(...)`.

The type cast also changes — old was `*mut lvgl_sys::lv_img_dsc_t as *const _`, new is `*mut lightvgl_sys::lv_image_dsc_t as *const _`.

### 6f: Remaining lvgl_sys:: references in main.rs

Do a final sweep — replace all remaining `lvgl_sys::` with `lightvgl_sys::`. Check:
- `lv_obj_create(core::ptr::null_mut())` (Screen 2 creation)
- `lv_label_create`, `lv_label_set_text`, `lv_obj_align`
- `LV_ALIGN_CENTER` constant
- `lv_timer_create` calls
- `lv_tick_inc`, `lv_timer_handler`
- Any remaining `lv_obj_set_pos` calls (note: was `i16` in v8, now `i32` in v9)

`lv_obj_set_pos` coordinates: In v8 `i16`, in v9 `i32`. Remove any `as i16` casts and use plain `i32` (or just the `i32` value).

**Step 1: Apply all 6a–6f changes**

**Step 2: Build**

```bash
cargo build 2>&1 | grep "^error" | head -60
```

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: update image widget and descriptor API to LVGL v9 in main.rs"
```

---

## Task 7: Update matrix_character.rs

**Files:**
- Modify: `src/matrix_character.rs`

**Step 1: Update lv_img_dsc_t fields in the MatrixCharacter struct (line 211-212)**

Old:
```rust
    idle_dscs: [lvgl_sys::lv_img_dsc_t; 2],
    walk_dscs: [lvgl_sys::lv_img_dsc_t; 4],
    widget: *mut lvgl_sys::lv_obj_t,
```
New:
```rust
    idle_dscs: [lightvgl_sys::lv_image_dsc_t; 2],
    walk_dscs: [lightvgl_sys::lv_image_dsc_t; 4],
    widget: *mut lightvgl_sys::lv_obj_t,
```

**Step 2: Update make_dsc helper (line 229-237)**

Old:
```rust
fn make_dsc(pixels: &'static [u16]) -> lvgl_sys::lv_img_dsc_t {
    let mut dsc = lvgl_sys::lv_img_dsc_t::default();
    dsc.header.set_cf(lvgl_sys::LV_IMG_CF_TRUE_COLOR as u32);
    dsc.header.set_w(CHAR_W as u32);
    dsc.header.set_h(CHAR_H as u32);
    dsc.data_size = (CHAR_W * CHAR_H * core::mem::size_of::<u16>() as i32) as u32;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}
```
New:
```rust
fn make_dsc(pixels: &'static [u16]) -> lightvgl_sys::lv_image_dsc_t {
    let mut dsc = lightvgl_sys::lv_image_dsc_t::default();
    dsc.header.cf = lightvgl_sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAP as u32;
    dsc.header.w = CHAR_W as u32;
    dsc.header.h = CHAR_H as u32;
    dsc.header.stride = CHAR_W as u32 * 2;  // bytes per row
    dsc.data_size = (CHAR_W * CHAR_H * 2) as u32;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}
```

**Step 3: Update MatrixCharacter::new() (line 246-287)**

- `lvgl_sys::lv_img_create(screen)` → `lightvgl_sys::lv_image_create(screen)`
- `lvgl_sys::lv_obj_set_pos(widget, x as i16, y as i16)` → `lightvgl_sys::lv_obj_set_pos(widget, x as i32, y as i32)`
- `lvgl_sys::lv_img_set_pivot(widget, 0, 0)` → `lightvgl_sys::lv_image_set_pivot(widget, 0, 0)`
- `lvgl_sys::lv_img_set_zoom(widget, 256 * scale as u16)` → `lightvgl_sys::lv_image_set_scale(widget, 256 * scale as u32)`
- `lvgl_sys::lv_img_set_src(...)` → `lightvgl_sys::lv_image_set_src(...)`
- Cast: `&this.idle_dscs[0] as *const lvgl_sys::lv_img_dsc_t as *const core::ffi::c_void` → `&this.idle_dscs[0] as *const lightvgl_sys::lv_image_dsc_t as *const core::ffi::c_void`

**Step 4: Update CharacterSprite impl — update() method (line 300-376)**

- All `lvgl_sys::lv_img_set_src(...)` → `lightvgl_sys::lv_image_set_src(...)`
- All `&self.idle_dscs[...] as *const lvgl_sys::lv_img_dsc_t` → `*const lightvgl_sys::lv_image_dsc_t`
- All `&self.walk_dscs[...] as *const lvgl_sys::lv_img_dsc_t` → `*const lightvgl_sys::lv_image_dsc_t`
- `lvgl_sys::lv_obj_set_pos(self.widget, self.pos_x as i16, self.pos_y as i16)` → `lightvgl_sys::lv_obj_set_pos(self.widget, self.pos_x, self.pos_y)`

**Step 5: Build**

```bash
cargo build 2>&1 | grep "^error" | head -60
```

**Step 6: Commit**

```bash
git add src/matrix_character.rs
git commit -m "feat: update matrix_character.rs to LVGL v9 image API"
```

---

## Task 8: Fix remaining build errors

**Files:**
- Whichever files have remaining errors

**Step 1: Run a full build and collect all errors**

```bash
cargo build 2>&1 | grep "^error\[" | sort | uniq
```

**Step 2: For each error, look up the binding name**

Check generated bindings to find correct names:
```bash
grep -i "lv_image_header\|lv_image_dsc\|set_cf\|set_w\|set_h\|stride\|LV_COLOR_FORMAT\|lv_display_render" \
  target/xtensa-esp32s3-espidf/debug/build/lightvgl-sys-*/out/bindings.rs | head -60
```

Common issues to watch for:
- `header.cf` may be a bitfield with setter `set_cf()` — use whatever the binding generates
- `header.stride` may not exist or may have a different name
- `lv_image_set_scale()` vs `lv_image_set_zoom()` — check which name lightvgl-sys exports
- `lv_indev_active()` vs `lv_indev_get_act()` — check binding
- Coordinate types: ensure no remaining `as i16` casts
- `lv_screen_load_anim` enum constant prefix may differ

**Step 3: Apply fixes**

For each error category, make the minimal fix to resolve it.

**Step 4: Full successful build**

```bash
cargo build 2>&1 | tail -5
```
Expected: `Finished dev [optimized + debuginfo] target(s) in ...`

**Step 5: Commit**

```bash
git add src/main.rs src/matrix_character.rs
git commit -m "fix: resolve remaining LVGL v9 binding name mismatches"
```

---

## Task 9: Flash and verify

**Step 1: Flash to device**

```bash
cargo espflash flash --monitor
```

**Step 2: Verify expected behavior**

- Display initializes (no blank screen)
- Spaceship background renders
- Crew/commander sprites animate
- Console blink works
- MatrixCharacter appears and walks
- Touch input responds (screen gesture swipe)
- No panics in log output

**Step 3: If display is blank or colors are wrong**

Check `LV_COLOR_FORMAT_RGB565_SWAP` is correct. The display and image descriptors must use the same color format. If colors are inverted/wrong:
- Try `LV_COLOR_FORMAT_RGB565` (without swap) on both display and image headers
- Or try swap on display but not on image headers — depends on whether pixel data in spaceship.rs/matrix_character.rs was stored pre-swapped

> Note: pixel art data in `spaceship.rs` has comment `// byte-swapped RGB565, LV_COLOR_16_SWAP=1`. This means the u16 values are stored with bytes swapped. In LVGL 9, setting the display to `RGB565_SWAP` means LVGL outputs swapped bytes — but internally renders in normal RGB565. The image data format should match what LVGL internally expects (normal RGB565 or swapped). If all colors look wrong, try setting image header `cf = LV_COLOR_FORMAT_RGB565` (no swap) and display format `LV_COLOR_FORMAT_RGB565_SWAP`.

**Step 4: Commit final state**

```bash
git add -p
git commit -m "feat: LVGL 9.5 migration complete — verified on hardware"
```

---

## Troubleshooting Reference

### "cannot find value `lv_image_set_scale` in module `lightvgl_sys`"
Check: `grep "lv_image_set_scale\|lv_image_set_zoom\|lv_img_set_zoom" target/*/debug/build/lightvgl-sys-*/out/bindings.rs`

### "no field `cf` on type `lv_image_header_t`"
The header fields are bitfields — use setter methods. Check: `grep "fn set_cf\|fn set_w\|fn set_h\|fn set_stride" target/*/debug/build/lightvgl-sys-*/out/bindings.rs`

### "mismatched types: expected i32, found i16"
Remove `as i16` casts — v9 coordinates are `i32`.

### Build fails with "string.h not found"
The `LIBCLANG_PATH` / `C_INCLUDE_PATH` vars in `.cargo/config.toml` should carry over from the old `lvgl-sys` setup. Ensure they're still pointing to the xtensa toolchain include dirs.

### Build fails with "cannot find -llvgl or similar"
`lightvgl-sys` compiles LVGL from source (same as `lvgl-sys` did). If it fails to find the C compiler, verify `CC_xtensa_esp32s3_espidf` is set in `.cargo/config.toml`.

### Colors look scrambled after successful boot
The `spaceship.rs` pixel data uses pre-swapped bytes (comment: `LV_COLOR_16_SWAP=1`). Set image header `cf = LV_COLOR_FORMAT_RGB565` and display format `LV_COLOR_FORMAT_RGB565_SWAP` — LVGL will handle the swap during rendering output.
