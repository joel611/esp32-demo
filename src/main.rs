use std::time::Duration;

mod ft3168;
mod safe_area;
mod character;
mod matrix_character;
mod spaceship;
mod sprites;

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;

extern "C" {
    fn lcd_driver_init() -> i32;
    #[allow(dead_code)]
    fn lcd_draw_bitmap(x1: i32, y1: i32, x2: i32, y2: i32, data: *const core::ffi::c_void);
    fn lcd_draw_bitmap_async(x1: i32, y1: i32, x2: i32, y2: i32, data: *const core::ffi::c_void);
    fn lcd_wait_flush_done();
}

const LCD_W: u32 = 466;
const LCD_H: u32 = 466;
const DRAW_BUF_PIXELS: usize = LCD_W as usize * 100; // 100 rows (~91KB internal DMA SRAM)

// Touch state written by the main loop, read by the LVGL indev callback.
// Both run on the same thread (indev cb is called inside lv_timer_handler),
// so Relaxed ordering is sufficient.
static TOUCH_X: AtomicI32 = AtomicI32::new(0);
static TOUCH_Y: AtomicI32 = AtomicI32::new(0);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);

// Screen object pointers. Written once during init, read by gesture callback.
static mut SCREEN1: *mut lightvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut SCREEN2: *mut lightvgl_sys::lv_obj_t = core::ptr::null_mut();

// Spaceship animation state — single-thread, written once during init
static mut CREW_WIDGETS: [*mut lightvgl_sys::lv_obj_t; 3] = [core::ptr::null_mut(); 3];
static mut CMD_WIDGET: *mut lightvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut BLINK_WIDGET: *mut lightvgl_sys::lv_obj_t = core::ptr::null_mut();

use crate::matrix_character::MatrixCharacter;
use crate::character::CharacterSprite;

// MatrixCharacter — lives for program lifetime (Box::leaked in ::new())
static mut MATRIX_CHAR: *mut MatrixCharacter = core::ptr::null_mut();

static mut CREW_FRAME: u8 = 0;
static mut CMD_FRAME: u8 = 0;
static mut BLINK_FRAME: u8 = 0;

// Image descriptors — must be 'static (LVGL holds raw pointers)
static mut CREW_DSC_A: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CREW_DSC_B: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_A:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_B:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut CMD_DSC_C:  *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BLINK_DSC_A: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BLINK_DSC_B: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();
static mut BG_DSC: *const lightvgl_sys::lv_image_dsc_t = core::ptr::null();

// LVGL flush callback — double-buffer async DMA pattern:
//   1. Wait for the previous async DMA to finish (no-op on first call).
//   2. Start a new async DMA for the current buffer.
//   3. Immediately signal flush_ready so LVGL can render into the OTHER buffer
//      while the DMA for this buffer runs in the background.
// lv_area_t coords are inclusive; lcd_draw_bitmap expects exclusive x2/y2.
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
    // Signal LVGL immediately: with two buffers, this swaps buf_act so LVGL
    // can render the next chunk into the other buffer concurrently with the DMA.
    lightvgl_sys::lv_display_flush_ready(disp);
}

/// LVGL input device read callback. Called by lv_timer_handler() on every tick.
/// Reads touch state from the atomics updated in the main loop.
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

/// Gesture event callback attached to both screens.
/// Swipe LEFT  → load screen 2 (if on screen 1).
/// Swipe RIGHT → load screen 1 (if on screen 2).
unsafe extern "C" fn gesture_cb(e: *mut lightvgl_sys::lv_event_t) {
    let indev = lightvgl_sys::lv_indev_active();
    if indev.is_null() {
        return;
    }
    let dir = lightvgl_sys::lv_indev_get_gesture_dir(indev);
    let active = lightvgl_sys::lv_screen_active();

    if dir == lightvgl_sys::lv_dir_t_LV_DIR_LEFT && active == SCREEN1 {
        lightvgl_sys::lv_screen_load_anim(
            SCREEN2,
            lightvgl_sys::lv_screen_load_anim_t_LV_SCREEN_LOAD_ANIM_MOVE_LEFT,
            150,
            0,
            false,
        );
    } else if dir == lightvgl_sys::lv_dir_t_LV_DIR_RIGHT && active == SCREEN2 {
        lightvgl_sys::lv_screen_load_anim(
            SCREEN1,
            lightvgl_sys::lv_screen_load_anim_t_LV_SCREEN_LOAD_ANIM_MOVE_RIGHT,
            150,
            0,
            false,
        );
    }

    // Suppress unused parameter warning
    let _ = e;
}

/// Build an lv_img_dsc_t for a u16 RGB565 pixel array.
/// w, h: sprite dimensions in pixels.
fn make_dsc(pixels: &'static [u16], w: u32, h: u32) -> lightvgl_sys::lv_image_dsc_t {
    let mut dsc: lightvgl_sys::lv_image_dsc_t = unsafe { core::mem::zeroed() };
    dsc.header.set_cf(lightvgl_sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAPPED as u32);
    dsc.header.set_w(w);
    dsc.header.set_h(h);
    dsc.header.set_stride(w * 2);  // bytes per row for RGB565
    dsc.data_size = w * h * 2;
    dsc.data = pixels.as_ptr() as *const u8;
    dsc
}

/// Crew animation timer — fires every 600 ms, cycles through 2 frames.
/// All 3 crew widgets share the same sprite frames (same body shape).
unsafe extern "C" fn crew_timer_cb(_timer: *mut lightvgl_sys::lv_timer_t) {
    CREW_FRAME = 1 - CREW_FRAME;
    let src = if CREW_FRAME == 0 { CREW_DSC_A } else { CREW_DSC_B };
    for i in 0..3 {
        lightvgl_sys::lv_image_set_src(CREW_WIDGETS[i], src as *const _);
    }
}

/// Commander animation timer — fires every 800 ms, cycles A→B→C→A.
unsafe extern "C" fn cmd_timer_cb(_timer: *mut lightvgl_sys::lv_timer_t) {
    CMD_FRAME = (CMD_FRAME + 1) % 3;
    let src = match CMD_FRAME {
        0 => CMD_DSC_A,
        1 => CMD_DSC_B,
        _ => CMD_DSC_C,
    };
    lightvgl_sys::lv_image_set_src(CMD_WIDGET, src as *const _);
}

/// Console blink timer — fires every 1200 ms, toggles between two colors.
unsafe extern "C" fn blink_timer_cb(_timer: *mut lightvgl_sys::lv_timer_t) {
    BLINK_FRAME = 1 - BLINK_FRAME;
    let src = if BLINK_FRAME == 0 { BLINK_DSC_A } else { BLINK_DSC_B };
    lightvgl_sys::lv_image_set_src(BLINK_WIDGET, src as *const _);
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("=== LVGL display test ===");

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

    // ── 1. Hardware init ──────────────────────────────────────────────────────
    let ret = unsafe { lcd_driver_init() };
    assert_eq!(ret, 0, "lcd_driver_init failed: {ret}");
    log::info!("lcd_driver_init OK");

    unsafe {
        // ── 2. LVGL init ──────────────────────────────────────────────────────
        lightvgl_sys::lv_init();

        // ── 3. Two DMA-capable pixel buffers for double-buffering ─────────────
        // Must be internal SRAM: esp-lcd SPI driver calls esp_ptr_dma_capable()
        // which rejects PSRAM. Two 100-row buffers (~182KB total).
        // In LVGL 9, lv_color_t is 3 bytes (RGB888) but draw buffers use the
        // display's color format (RGB565_SWAPPED = 2 bytes/pixel).
        let pixel_size = 2_usize; // RGB565 = 2 bytes per pixel
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
            lightvgl_sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAPPED,
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

        // ── 6. Input device (touch) ───────────────────────────────────────────
        let indev = lightvgl_sys::lv_indev_create();
        assert!(!indev.is_null(), "lv_indev_create failed");
        lightvgl_sys::lv_indev_set_type(
            indev,
            lightvgl_sys::lv_indev_type_t_LV_INDEV_TYPE_POINTER,
        );
        lightvgl_sys::lv_indev_set_read_cb(indev, Some(lvgl_touch_cb));
        log::info!("LVGL touch input registered");

        // ── 7. Two-screen UI ──────────────────────────────────────────────────
        // Screen 1: the default screen LVGL created when the display was registered.
        SCREEN1 = lightvgl_sys::lv_screen_active();

        // ── Screen 1: Spaceship bridge scene ─────────────────────────────────────

        // Black base background for the screen
        lightvgl_sys::lv_obj_set_style_bg_color(
            SCREEN1,
            lightvgl_sys::lv_color_make(0x1a, 0x20, 0x40),
            lightvgl_sys::lv_state_t_LV_STATE_DEFAULT,
        );

        // ── Background image (466×466) ────────────────────────────────────────────
        let bg_dsc = Box::leak(Box::new(make_dsc(
            &spaceship::BG_FRAME,
            466,
            466,
        )));
        BG_DSC = bg_dsc as *const _;
        let bg_img = lightvgl_sys::lv_image_create(SCREEN1);
        lightvgl_sys::lv_image_set_src(bg_img, bg_dsc as *mut lightvgl_sys::lv_image_dsc_t as *const _);
        lightvgl_sys::lv_obj_set_pos(bg_img, 0, 0);

        // ── Crew descriptors (shared frames for all 3 crew) ───────────────────────
        let crew_a_dsc = Box::leak(Box::new(make_dsc(
            &spaceship::CREW_FRAME_A,
            spaceship::CREW_W as u32,
            spaceship::CREW_H as u32,
        )));
        let crew_b_dsc = Box::leak(Box::new(make_dsc(
            &spaceship::CREW_FRAME_B,
            spaceship::CREW_W as u32,
            spaceship::CREW_H as u32,
        )));
        CREW_DSC_A = crew_a_dsc as *const _;
        CREW_DSC_B = crew_b_dsc as *const _;

        // ── Crew widgets: (x, y) top-left of sprite ───────────────────────────────
        // Positions: center sprite over these display coords.
        // Crew #1 (left):        center at ( 80, 100)
        // Crew #3 (back-center): center at (210,  80)
        // Crew #2 (right):       center at (340, 100)
        let crew_positions: [(i32, i32); 3] = [
            ( 80 - spaceship::CREW_W / 2, 100 - spaceship::CREW_H / 2), // crew #1 left
            (210 - spaceship::CREW_W / 2,  80 - spaceship::CREW_H / 2), // crew #3 back-center
            (340 - spaceship::CREW_W / 2, 100 - spaceship::CREW_H / 2), // crew #2 right
        ];
        for i in 0..3 {
            let w = lightvgl_sys::lv_image_create(SCREEN1);
            lightvgl_sys::lv_image_set_src(w, crew_a_dsc as *mut lightvgl_sys::lv_image_dsc_t as *const _);
            lightvgl_sys::lv_obj_set_pos(w, crew_positions[i].0, crew_positions[i].1);
            CREW_WIDGETS[i] = w;
        }

        // ── Commander descriptor and widget ──────────────────────────────────────
        let cmd_a_dsc = Box::leak(Box::new(make_dsc(&spaceship::CMD_FRAME_A, spaceship::CMD_W as u32, spaceship::CMD_H as u32)));
        let cmd_b_dsc = Box::leak(Box::new(make_dsc(&spaceship::CMD_FRAME_B, spaceship::CMD_W as u32, spaceship::CMD_H as u32)));
        let cmd_c_dsc = Box::leak(Box::new(make_dsc(&spaceship::CMD_FRAME_C, spaceship::CMD_W as u32, spaceship::CMD_H as u32)));
        CMD_DSC_A = cmd_a_dsc as *const _;
        CMD_DSC_B = cmd_b_dsc as *const _;
        CMD_DSC_C = cmd_c_dsc as *const _;

        // Commander center at (205, 340); sprite top-left:
        let cmd_widget = lightvgl_sys::lv_image_create(SCREEN1);
        lightvgl_sys::lv_image_set_src(cmd_widget, cmd_a_dsc as *mut lightvgl_sys::lv_image_dsc_t as *const _);
        lightvgl_sys::lv_obj_set_pos(cmd_widget,
            205 - spaceship::CMD_W / 2,
            340 - spaceship::CMD_H / 2,
        );
        CMD_WIDGET = cmd_widget;

        // ── Console blink widget ──────────────────────────────────────────────────
        let blink_a_dsc = Box::leak(Box::new(make_dsc(&spaceship::BLINK_FRAME_A, spaceship::BLINK_W as u32, spaceship::BLINK_H as u32)));
        let blink_b_dsc = Box::leak(Box::new(make_dsc(&spaceship::BLINK_FRAME_B, spaceship::BLINK_W as u32, spaceship::BLINK_H as u32)));
        BLINK_DSC_A = blink_a_dsc as *const _;
        BLINK_DSC_B = blink_b_dsc as *const _;

        // Position inside the back-center console screen area (x:183..283, y:30..95)
        // Blink widget at (193, 35) — 20×10 overlay
        let blink_widget = lightvgl_sys::lv_image_create(SCREEN1);
        lightvgl_sys::lv_image_set_src(blink_widget, blink_a_dsc as *mut lightvgl_sys::lv_image_dsc_t as *const _);
        lightvgl_sys::lv_obj_set_pos(blink_widget, 193, 35);
        BLINK_WIDGET = blink_widget;

        // ── Matrix character ─────────────────────────────────────────────────────────
        // Spawn at centre of screen; call walk_to() to move it.
        MATRIX_CHAR = MatrixCharacter::new(SCREEN1, 200, 200, 3);
        log::info!("MatrixCharacter created at (200, 200)");

        // Quick smoke test: walk from (200,200) to (100,350).
        (*MATRIX_CHAR).walk_to(100, 350);

        // ── Animation timers ──────────────────────────────────────────────────────
        lightvgl_sys::lv_timer_create(Some(crew_timer_cb),  600,  core::ptr::null_mut());
        lightvgl_sys::lv_timer_create(Some(cmd_timer_cb),   800,  core::ptr::null_mut());
        lightvgl_sys::lv_timer_create(Some(blink_timer_cb), 1200, core::ptr::null_mut());

        // Screen 2: new screen object (parent = null → creates a standalone screen)
        SCREEN2 = lightvgl_sys::lv_obj_create(core::ptr::null_mut());

        // Blue background for screen 2 so it's visually distinct
        lightvgl_sys::lv_obj_set_style_bg_color(
            SCREEN2,
            lightvgl_sys::lv_color_make(0x00, 0x30, 0x80),
            lightvgl_sys::lv_state_t_LV_STATE_DEFAULT,
        );

        let label2 = lightvgl_sys::lv_label_create(SCREEN2);
        lightvgl_sys::lv_label_set_text(label2, b"Screen 2\0".as_ptr());
        lightvgl_sys::lv_obj_align(label2, lightvgl_sys::lv_align_t_LV_ALIGN_CENTER, 0, 0);

        // Attach gesture callbacks — LVGL sends LV_EVENT_GESTURE to the screen
        // when a drag exceeds LV_INDEV_DEF_GESTURE_LIMIT (default 50px).
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

        log::info!("Two screens created, gesture callbacks attached");
    }

    // ── 8. LVGL timer loop ────────────────────────────────────────────────────
    log::info!("Entering LVGL loop");
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
            lightvgl_sys::lv_tick_inc(5);
            lightvgl_sys::lv_timer_handler();
            // Advance MatrixCharacter animation (idle blink / walk frames + position).
            (*MATRIX_CHAR).update(5);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
