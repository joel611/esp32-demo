// Two RGB565 64×64 frames for Pikachu idle animation.
//
// LV_COLOR_16_SWAP=1 is active. The internal lv_color_t byte layout is:
//   bits  0–2 : green_h (upper 3 of 6-bit green)
//   bits  3–7 : red (5 bits)
//   bits  8–12: blue (5 bits)
//   bits 13–15: green_l (lower 3 of 6-bit green)
//
// Yellow (R=255, G=255, B=0): green_h=7|red=31|blue=0|green_l=7
//   = 0b111_11111_00000_111 → as u16 little-endian = 0xE0FF
//
// PLACEHOLDER DATA: solid color fills so widget layout can be verified before
// adding real art. Replace with proper 64×64 Pikachu sprite later.
// To convert PNG → RGB565 array: use https://lvgl.io/tools/imageconverter
// (select CF_TRUE_COLOR, 16-bit color, swap bytes).

pub static PIKACHU_FRAME_A: [u16; 4096] = [0xE0FF; 4096]; // yellow — idle pose
pub static PIKACHU_FRAME_B: [u16; 4096] = [0xC0FF; 4096]; // amber — blink/twitch
