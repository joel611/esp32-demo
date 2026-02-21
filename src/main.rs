use std::time::Duration;

mod ft3168;
mod safe_area;
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
static mut SCREEN1: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut SCREEN2: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
// Animation state — written once during init, accessed only on the main thread.
static mut IMG_WIDGET: *mut lvgl_sys::lv_obj_t = core::ptr::null_mut();
static mut FRAME_IDX: u8 = 0;
static mut IMG_A_DSC: *const lvgl_sys::lv_img_dsc_t = core::ptr::null();
static mut IMG_B_DSC: *const lvgl_sys::lv_img_dsc_t = core::ptr::null();

// LVGL flush callback — double-buffer async DMA pattern:
//   1. Wait for the previous async DMA to finish (no-op on first call).
//   2. Start a new async DMA for the current buffer.
//   3. Immediately signal flush_ready so LVGL can render into the OTHER buffer
//      while the DMA for this buffer runs in the background.
// lv_area_t coords are inclusive; lcd_draw_bitmap expects exclusive x2/y2.
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
    // Signal LVGL immediately: with two buffers, this swaps buf_act so LVGL
    // can render the next chunk into the other buffer concurrently with the DMA.
    lvgl_sys::lv_disp_flush_ready(disp_drv);
}

/// LVGL input device read callback. Called by lv_timer_handler() on every tick.
/// Reads touch state from the atomics updated in the main loop.
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

/// Gesture event callback attached to both screens.
/// Swipe LEFT  → load screen 2 (if on screen 1).
/// Swipe RIGHT → load screen 1 (if on screen 2).
unsafe extern "C" fn gesture_cb(e: *mut lvgl_sys::lv_event_t) {
    let indev = lvgl_sys::lv_indev_get_act();
    if indev.is_null() {
        return;
    }
    let dir = lvgl_sys::lv_indev_get_gesture_dir(indev); // returns lv_dir_t = u8
    let active = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());

    if dir == lvgl_sys::LV_DIR_LEFT as lvgl_sys::lv_dir_t && active == SCREEN1 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN2,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_LEFT,
            150,
            0,
            false,
        );
    } else if dir == lvgl_sys::LV_DIR_RIGHT as lvgl_sys::lv_dir_t && active == SCREEN2 {
        lvgl_sys::lv_scr_load_anim(
            SCREEN1,
            lvgl_sys::lv_scr_load_anim_t_LV_SCR_LOAD_ANIM_MOVE_RIGHT,
            150,
            0,
            false,
        );
    }

    // Suppress unused parameter warning
    let _ = e;
}

/// LVGL timer callback — fires every 600 ms to toggle the sprite frame.
/// Called by lv_timer_handler() on the main thread; all statics are single-thread.
unsafe extern "C" fn anim_timer_cb(_timer: *mut lvgl_sys::lv_timer_t) {
    FRAME_IDX = 1 - FRAME_IDX;
    log::debug!("anim_timer: frame={}", FRAME_IDX);   // ← add this line
    let src = if FRAME_IDX == 0 { IMG_A_DSC } else { IMG_B_DSC };
    lvgl_sys::lv_img_set_src(IMG_WIDGET, src as *const _);
}

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
        lvgl_sys::lv_init();

        // ── 3. Two DMA-capable pixel buffers for double-buffering ─────────────
        // Must be internal SRAM: esp-lcd SPI driver calls esp_ptr_dma_capable()
        // which rejects PSRAM. Two 100-row buffers (~182KB total).
        let pixel_size = core::mem::size_of::<lvgl_sys::lv_color_t>();
        let buf1 = esp_idf_svc::sys::heap_caps_malloc(
            DRAW_BUF_PIXELS * pixel_size,
            esp_idf_svc::sys::MALLOC_CAP_DMA,
        ) as *mut lvgl_sys::lv_color_t;
        let buf2 = esp_idf_svc::sys::heap_caps_malloc(
            DRAW_BUF_PIXELS * pixel_size,
            esp_idf_svc::sys::MALLOC_CAP_DMA,
        ) as *mut lvgl_sys::lv_color_t;
        assert!(!buf1.is_null() && !buf2.is_null(), "LVGL draw buf alloc failed");

        // ── 4. Draw buffer struct (leaked: LVGL holds a pointer to it) ────────
        let disp_buf: &'static mut lvgl_sys::lv_disp_draw_buf_t =
            Box::leak(Box::new(core::mem::zeroed()));
        lvgl_sys::lv_disp_draw_buf_init(
            disp_buf,
            buf1 as *mut _,
            buf2 as *mut _,
            DRAW_BUF_PIXELS as u32,
        );

        // ── 5. Display driver (leaked: LVGL 8.x stores the pointer, not a copy) ─
        let disp_drv: &'static mut lvgl_sys::lv_disp_drv_t =
            Box::leak(Box::new(core::mem::zeroed()));
        lvgl_sys::lv_disp_drv_init(disp_drv);
        disp_drv.hor_res = LCD_W as lvgl_sys::lv_coord_t;
        disp_drv.ver_res = LCD_H as lvgl_sys::lv_coord_t;
        disp_drv.draw_buf = disp_buf;
        disp_drv.flush_cb = Some(lvgl_flush_cb);
        lvgl_sys::lv_disp_drv_register(disp_drv);
        log::info!("LVGL display registered");

        // ── 6. Input device (touch) ───────────────────────────────────────────
        let indev_drv: &'static mut lvgl_sys::lv_indev_drv_t =
            Box::leak(Box::new(core::mem::zeroed()));
        lvgl_sys::lv_indev_drv_init(indev_drv);
        indev_drv.type_ = lvgl_sys::lv_indev_type_t_LV_INDEV_TYPE_POINTER;
        indev_drv.read_cb = Some(lvgl_touch_cb);
        lvgl_sys::lv_indev_drv_register(indev_drv);
        log::info!("LVGL touch input registered");

        // ── 7. Two-screen UI ──────────────────────────────────────────────────
        // Screen 1: the default screen LVGL created when the display was registered.
        SCREEN1 = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());

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
        lvgl_sys::lv_img_set_src(img_widget, img_a as *mut lvgl_sys::lv_img_dsc_t as *const _);
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

        // Screen 2: new screen object (parent = null → creates a standalone screen)
        SCREEN2 = lvgl_sys::lv_obj_create(core::ptr::null_mut());

        // Blue background for screen 2 so it's visually distinct
        lvgl_sys::lv_obj_set_style_bg_color(
            SCREEN2,
            lvgl_sys::_LV_COLOR_MAKE(0x00, 0x30, 0x80),
            lvgl_sys::LV_STATE_DEFAULT,
        );

        let label2 = lvgl_sys::lv_label_create(SCREEN2);
        lvgl_sys::lv_label_set_text(label2, b"Screen 2\0".as_ptr() as *const i8);
        lvgl_sys::lv_obj_align(label2, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 0);

        // Attach gesture callbacks — LVGL sends LV_EVENT_GESTURE to the screen
        // when a drag exceeds LV_INDEV_DEF_GESTURE_LIMIT (default 50px).
        lvgl_sys::lv_obj_add_event_cb(
            SCREEN1,
            Some(gesture_cb),
            lvgl_sys::lv_event_code_t_LV_EVENT_GESTURE,
            core::ptr::null_mut(),
        );
        lvgl_sys::lv_obj_add_event_cb(
            SCREEN2,
            Some(gesture_cb),
            lvgl_sys::lv_event_code_t_LV_EVENT_GESTURE,
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
            lvgl_sys::lv_tick_inc(5);
            lvgl_sys::lv_timer_handler();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
