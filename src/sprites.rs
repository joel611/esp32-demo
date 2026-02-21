// Two RGB565 64×64 frames for Pikachu idle animation.
// LV_COLOR_16_SWAP=1 is active: all color values have byte-swap applied.
//   BG=0x0000 (black)  YL=0xE0FF (yellow)  RD=0x00F8 (red)  WH=0xFFFF (white)

const BG: u16 = 0x0000;
const YL: u16 = 0xE0FF;
const RD: u16 = 0x00F8;
const WH: u16 = 0xFFFF;

const fn iabs(n: i32) -> i32 {
    if n < 0 { -n } else { n }
}

/// True if (x, y) lies inside the ellipse centred at (cx, cy)
/// with horizontal semi-axis `a` and vertical semi-axis `b`.
const fn in_ellipse(x: i32, y: i32, cx: i32, cy: i32, a: i32, b: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    // avoids floating-point: (dx/a)^2 + (dy/b)^2 <= 1
    dx * dx * b * b + dy * dy * a * a <= a * a * b * b
}

// ─── Frame A: eyes open ──────────────────────────────────────────────────────
const fn pixel_a(x: i32, y: i32) -> u16 {
    // Ears: upward triangles, tips at x=14 and x=50
    let in_left_ear  = y <= 20 && iabs(x - 14) <= y / 2 + 2;
    let in_right_ear = y <= 20 && iabs(x - 50) <= y / 2 + 2;
    // Inner ear stripe (rendered dark)
    let in_left_ear_inner  = y >= 2 && y <= 18 && iabs(x - 14) <= y / 3;
    let in_right_ear_inner = y >= 2 && y <= 18 && iabs(x - 50) <= y / 3;

    // Head: ellipse centred at (32, 36), rx=26, ry=24
    let in_head = in_ellipse(x, y, 32, 36, 26, 24);

    // Eyes: filled ellipse (open)
    let in_left_eye  = in_ellipse(x, y, 21, 29, 5, 6);
    let in_right_eye = in_ellipse(x, y, 43, 29, 5, 6);
    // Eye-shine
    let in_left_hl  = in_ellipse(x, y, 23, 27, 2, 2);
    let in_right_hl = in_ellipse(x, y, 45, 27, 2, 2);

    // Red cheeks
    let in_left_cheek  = in_ellipse(x, y, 17, 40, 7, 7);
    let in_right_cheek = in_ellipse(x, y, 47, 40, 7, 7);

    // Nose (tiny horizontal mark)
    let in_nose = x >= 30 && x <= 33 && y == 33;

    // Mouth (curved smile)
    let in_mouth = (y == 37 && x >= 27 && x <= 37)
        || (y == 38 && (x == 26 || x == 38))
        || (y == 39 && (x == 25 || x == 39));

    if in_head || in_left_ear || in_right_ear {
        if in_left_ear_inner || in_right_ear_inner {
            BG
        } else if in_left_eye || in_right_eye {
            if in_left_hl || in_right_hl { WH } else { BG }
        } else if in_nose || in_mouth {
            BG
        } else if in_left_cheek || in_right_cheek {
            RD
        } else {
            YL
        }
    } else {
        BG
    }
}

// ─── Frame B: eyes closed (blink) ────────────────────────────────────────────
const fn pixel_b(x: i32, y: i32) -> u16 {
    let in_left_ear  = y <= 20 && iabs(x - 14) <= y / 2 + 2;
    let in_right_ear = y <= 20 && iabs(x - 50) <= y / 2 + 2;
    let in_left_ear_inner  = y >= 2 && y <= 18 && iabs(x - 14) <= y / 3;
    let in_right_ear_inner = y >= 2 && y <= 18 && iabs(x - 50) <= y / 3;
    let in_head = in_ellipse(x, y, 32, 36, 26, 24);

    // Eyes closed: short horizontal line where eyes were
    let in_left_eye_closed  = y == 30 && x >= 18 && x <= 24;
    let in_right_eye_closed = y == 30 && x >= 40 && x <= 46;

    let in_left_cheek  = in_ellipse(x, y, 17, 40, 7, 7);
    let in_right_cheek = in_ellipse(x, y, 47, 40, 7, 7);
    let in_nose = x >= 30 && x <= 33 && y == 33;
    let in_mouth = (y == 37 && x >= 27 && x <= 37)
        || (y == 38 && (x == 26 || x == 38))
        || (y == 39 && (x == 25 || x == 39));

    if in_head || in_left_ear || in_right_ear {
        if in_left_ear_inner || in_right_ear_inner {
            BG
        } else if in_left_eye_closed || in_right_eye_closed {
            BG
        } else if in_nose || in_mouth {
            BG
        } else if in_left_cheek || in_right_cheek {
            RD
        } else {
            YL
        }
    } else {
        BG
    }
}

const fn make_frame_a() -> [u16; 4096] {
    let mut p = [BG; 4096];
    let mut y: i32 = 0;
    while y < 64 {
        let mut x: i32 = 0;
        while x < 64 {
            p[(y * 64 + x) as usize] = pixel_a(x, y);
            x += 1;
        }
        y += 1;
    }
    p
}

const fn make_frame_b() -> [u16; 4096] {
    let mut p = [BG; 4096];
    let mut y: i32 = 0;
    while y < 64 {
        let mut x: i32 = 0;
        while x < 64 {
            p[(y * 64 + x) as usize] = pixel_b(x, y);
            x += 1;
        }
        y += 1;
    }
    p
}

pub static PIKACHU_FRAME_A: [u16; 4096] = make_frame_a();
pub static PIKACHU_FRAME_B: [u16; 4096] = make_frame_b();
