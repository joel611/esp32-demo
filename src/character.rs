// src/character.rs
// Abstract interface for animated, moveable pixel-art characters.

/// Implemented by any character that has idle and walk animations
/// and can be moved to a target position on screen.
pub trait CharacterSprite {
    /// Begin walking toward `(target_x, target_y)` (top-left of sprite).
    /// No-op if already at that position.
    fn walk_to(&mut self, target_x: i32, target_y: i32);

    /// Advance animation frames and position by `delta_ms` milliseconds.
    /// Call this every tick from the LVGL loop (typically every 5 ms).
    fn update(&mut self, delta_ms: u32);

    /// Current top-left position on screen.
    fn position(&self) -> (i32, i32);

    /// True when the character is not walking.
    fn is_idle(&self) -> bool;
}
