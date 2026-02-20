use std::time::Duration;

mod ft3168;

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;

extern "C" {
    fn lcd_driver_init() -> i32;
    fn lcd_draw_bitmap(x1: i32, y1: i32, x2: i32, y2: i32, data: *const core::ffi::c_void);
}

const LCD_W: u32 = 466;
const LCD_H: u32 = 466;
const DRAW_BUF_PIXELS: usize = LCD_W as usize * 20; // 20 rows per flush

// Touch state written by the main loop, read by the LVGL indev callback.
// Both run on the same thread (indev cb is called inside lv_timer_handler),
// so Relaxed ordering is sufficient.
static TOUCH_X: AtomicI32 = AtomicI32::new(0);
static TOUCH_Y: AtomicI32 = AtomicI32::new(0);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);

// LVGL flush callback: called by LVGL when a region needs to be sent to the display.
// lv_area_t coords are inclusive; lcd_draw_bitmap expects exclusive x2/y2.
unsafe extern "C" fn lvgl_flush_cb(
    disp_drv: *mut lvgl_sys::lv_disp_drv_t,
    area: *const lvgl_sys::lv_area_t,
    color_p: *mut lvgl_sys::lv_color_t,
) {
    let x1 = (*area).x1 as i32;
    let y1 = (*area).y1 as i32;
    let x2 = (*area).x2 as i32 + 1;
    let y2 = (*area).y2 as i32 + 1;
    lcd_draw_bitmap(x1, y1, x2, y2, color_p as *const _);
    // lcd_draw_bitmap blocks until DMA completes, so we can signal ready immediately.
    lvgl_sys::lv_disp_flush_ready(disp_drv);
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

        // ── 3. DMA-capable pixel buffer ───────────────────────────────────────
        let buf = esp_idf_svc::sys::heap_caps_malloc(
            DRAW_BUF_PIXELS * core::mem::size_of::<lvgl_sys::lv_color_t>(),
            esp_idf_svc::sys::MALLOC_CAP_DMA,
        ) as *mut lvgl_sys::lv_color_t;
        assert!(!buf.is_null(), "LVGL draw buf alloc failed");

        // ── 4. Draw buffer struct (leaked: LVGL holds a pointer to it) ────────
        let disp_buf: &'static mut lvgl_sys::lv_disp_draw_buf_t =
            Box::leak(Box::new(core::mem::zeroed()));
        lvgl_sys::lv_disp_draw_buf_init(
            disp_buf,
            buf as *mut _,
            core::ptr::null_mut(),
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

        // ── 6. Simple UI: centered label ──────────────────────────────────────
        let screen = lvgl_sys::lv_disp_get_scr_act(lvgl_sys::lv_disp_get_default());
        let label = lvgl_sys::lv_label_create(screen);
        lvgl_sys::lv_label_set_text(label, b"Hello ESP32!\0".as_ptr() as *const i8);
        lvgl_sys::lv_obj_align(label, lvgl_sys::LV_ALIGN_CENTER as u8, 0, 0);
        log::info!("UI created");
    }

    // ── 7. LVGL timer loop ────────────────────────────────────────────────────
    log::info!("Entering LVGL loop");
    loop {
        unsafe {
            lvgl_sys::lv_tick_inc(5);
            lvgl_sys::lv_timer_handler();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
