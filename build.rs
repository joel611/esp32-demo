use std::io::Write;

fn rgb565_swap(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g6 = (g >> 2) as u16;
    let b5 = (b >> 3) as u16;
    let rgb = (r5 << 11) | (g6 << 5) | b5;
    (rgb >> 8) | (rgb << 8) // byte-swap for LV_COLOR_16_SWAP=1
}

fn pixel_bg(x: i32, y: i32) -> u16 {
    let bg_dark    = rgb565_swap(0x1a, 0x20, 0x40); // dark navy floor
    let wall       = rgb565_swap(0x2a, 0x34, 0x60); // back/side walls
    let console    = rgb565_swap(0x1c, 0x2e, 0x3a); // console panel body
    let screen_blu = rgb565_swap(0x00, 0xc8, 0xff); // monitor cyan
    let screen_grn = rgb565_swap(0x00, 0xff, 0x88); // readout green
    let metal      = rgb565_swap(0x50, 0x60, 0xa0); // metal trim
    let star_wht   = rgb565_swap(0xff, 0xff, 0xff); // stars

    // ── Regions ──────────────────────────────────────────────────────────────
    let in_back_wall = y < 150;

    // Windows (upper-left and upper-right)
    let in_left_win  = x >= 20  && x <= 140 && y >= 10 && y <= 145;
    let in_right_win = x >= 326 && x <= 446 && y >= 10 && y <= 145;
    let in_window = in_left_win || in_right_win;

    // Stars: deterministic scatter using hash
    let is_star = in_window && ((x * 7 + y * 13 + x * y / 3) % 31 == 0);

    // Window inner border (1-2px dark frame)
    let on_win_border = in_window && (
        x == 20 || x == 140 || x == 326 || x == 446 ||
        y == 10 || y == 145
    );

    // ── Back-center console: x 150..316, y 15..120 ────────────────────────
    let in_back_con  = x >= 150 && x <= 316 && y >= 15 && y <= 120;
    let in_back_scr  = x >= 183 && x <= 283 && y >= 30 && y <= 95; // cyan screen
    let on_back_con_border = in_back_con && !in_back_scr && (
        x == 150 || x == 316 || y == 15 || y == 120 ||
        x == 151 || x == 315 || y == 16 || y == 119
    );

    // Side console readouts inside back console (green strips)
    let in_back_grn_l = x >= 155 && x <= 178 && y >= 30 && y <= 95;
    let in_back_grn_r = x >= 288 && x <= 311 && y >= 30 && y <= 95;

    // ── Left console: x 15..165, y 195..295 ───────────────────────────────
    let in_left_con  = x >= 15  && x <= 165 && y >= 195 && y <= 295;
    let in_left_scr  = x >= 40  && x <= 140 && y >= 210 && y <= 270;
    let on_left_con_border = in_left_con && !in_left_scr && (
        x == 15 || x == 165 || y == 195 || y == 295 ||
        x == 16 || x == 164 || y == 196 || y == 294
    );
    let in_left_grn  = x >= 145 && x <= 162 && y >= 210 && y <= 270;

    // ── Right console: x 301..451, y 195..295 ─────────────────────────────
    let in_right_con  = x >= 301 && x <= 451 && y >= 195 && y <= 295;
    let in_right_scr  = x >= 326 && x <= 426 && y >= 210 && y <= 270;
    let on_right_con_border = in_right_con && !in_right_scr && (
        x == 301 || x == 451 || y == 195 || y == 295 ||
        x == 302 || x == 450 || y == 196 || y == 294
    );
    let in_right_grn  = x >= 304 && x <= 321 && y >= 210 && y <= 270;

    // ── Commander console: x 130..336, y 370..445 ─────────────────────────
    let in_cmd_con   = x >= 130 && x <= 336 && y >= 370 && y <= 445;
    let in_cmd_scr   = x >= 163 && x <= 303 && y >= 385 && y <= 430;
    let on_cmd_con_border = in_cmd_con && !in_cmd_scr && (
        x == 130 || x == 336 || y == 370 || y == 445 ||
        x == 131 || x == 335 || y == 371 || y == 444
    );
    let in_cmd_grn_l = x >= 135 && x <= 158 && y >= 385 && y <= 430;
    let in_cmd_grn_r = x >= 308 && x <= 331 && y >= 385 && y <= 430;

    // ── Floor panel grid (subtle lines) ───────────────────────────────────
    let on_floor_grid = y >= 145 && !in_left_con && !in_right_con && !in_cmd_con
        && (x % 46 == 0 || y % 46 == 0);

    // ── Rendering (priority: front features first) ─────────────────────────
    if in_back_scr   { screen_blu }
    else if in_back_grn_l || in_back_grn_r { screen_grn }
    else if on_back_con_border { metal }
    else if in_back_con { console }

    else if in_left_scr  { screen_blu }
    else if in_left_grn  { screen_grn }
    else if on_left_con_border  { metal }
    else if in_left_con  { console }

    else if in_right_scr { screen_blu }
    else if in_right_grn { screen_grn }
    else if on_right_con_border { metal }
    else if in_right_con { console }

    else if in_cmd_scr   { screen_blu }
    else if in_cmd_grn_l || in_cmd_grn_r { screen_grn }
    else if on_cmd_con_border  { metal }
    else if in_cmd_con   { console }

    else if is_star        { star_wht }
    else if on_win_border  { metal }
    else if in_window      { bg_dark } // dark space through windows

    else if on_floor_grid  { wall }   // subtle grid lines
    else if in_back_wall   { wall }   // back wall darker than floor
    else { bg_dark }                  // floor
}

fn generate_spaceship_bg() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out_dir).join("spaceship_bg.rs");
    let mut f = std::fs::File::create(&dest).unwrap();

    let w = 466i32;
    let h = 466i32;
    let n = (w * h) as usize;

    writeln!(f, "pub static BG_FRAME: [u16; {n}] = [").unwrap();
    for y in 0..h {
        for x in 0..w {
            write!(f, "{},", pixel_bg(x, y)).unwrap();
        }
    }
    writeln!(f, "];").unwrap();
}

fn main() {
    embuild::espidf::sysenv::output();
    generate_spaceship_bg();
}
