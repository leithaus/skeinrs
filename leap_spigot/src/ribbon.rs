//! Ribbon visualization state.
//!
//! Each ribbon is a circular buffer of colored digit-patches.  The visual
//! state tracks scrolling, stitching (when playing), and highlighting
//! (when snipping).

// ════════════════════════════════════════════════════════════════════════════
// Color palette — digit → RGB
// ════════════════════════════════════════════════════════════════════════════

/// Map a digit (0–35) to an ARGB color for the ribbon patch.
///
/// We use a perceptually-spaced hue wheel so adjacent digits have distinct
/// colors.  Base-10 cycles through 10 hues; higher bases extend into more
/// colors seamlessly.
pub fn digit_color(d: u8, base: u8) -> u32 {
    // Hue: distribute evenly around 360°
    let hue   = (d as f32 / base.max(1) as f32) * 360.0;
    let sat   = 0.82_f32;
    let val   = 0.92_f32;
    hsv_to_argb(hue, sat, val)
}

/// Convert HSV → packed ARGB (0xAARRGGBB, A=0xFF).
fn hsv_to_argb(h: f32, s: f32, v: f32) -> u32 {
    let h  = h % 360.0;
    let hi = (h / 60.0) as u32;
    let f  = h / 60.0 - hi as f32;
    let p  = v * (1.0 - s);
    let q  = v * (1.0 - s * f);
    let t  = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let ri = (r * 255.0) as u32;
    let gi = (g * 255.0) as u32;
    let bi = (b * 255.0) as u32;
    0xFF000000 | (ri << 16) | (gi << 8) | bi
}

// ════════════════════════════════════════════════════════════════════════════
// Patch — a single cell in the ribbon
// ════════════════════════════════════════════════════════════════════════════

/// A single digit-patch on the ribbon.
#[derive(Clone, Debug)]
pub struct Patch {
    pub digit:    u8,
    pub color:    u32,
    /// Stream position (0-based digit index in the original spigot).
    pub position: usize,
}

// ════════════════════════════════════════════════════════════════════════════
// RibbonState — the data behind one ribbon
// ════════════════════════════════════════════════════════════════════════════

/// Circular buffer of visible patches for one stream ribbon.
///
/// `capacity` patches are kept; the head always shows the most-recently
/// generated digit on the right, and the ribbon scrolls left as new digits
/// arrive.
#[derive(Debug)]
pub struct RibbonState {
    pub patches:  Vec<Patch>,
    pub capacity: usize,
    pub base:     u8,
    /// Sub-pixel scroll offset for smooth animation (pixels).
    pub scroll_px: f32,
    /// Scroll velocity in pixels/frame; set by pull gesture.
    pub scroll_vel: f32,
    /// Label for display (e.g. "π base 16")
    pub label:    String,
}

impl RibbonState {
    pub fn new(capacity: usize, base: u8, label: &str) -> Self {
        RibbonState {
            patches:    Vec::with_capacity(capacity),
            capacity,
            base,
            scroll_px:  0.0,
            scroll_vel: 0.0,
            label:      label.to_string(),
        }
    }

    /// Push a new digit onto the right end of the ribbon (oldest falls off left).
    pub fn push(&mut self, digit: u8, position: usize) {
        if self.patches.len() >= self.capacity {
            self.patches.remove(0);
        }
        self.patches.push(Patch {
            digit,
            color: digit_color(digit, self.base),
            position,
        });
    }

    /// Advance the scroll animation by one frame.
    /// `patch_width` is the pixel width of each patch.
    pub fn tick(&mut self, patch_width: f32) {
        self.scroll_px += self.scroll_vel;
        // Snap once a full patch has scrolled past
        while self.scroll_px >= patch_width {
            self.scroll_px -= patch_width;
        }
        // Friction
        self.scroll_vel *= 0.88;
        if self.scroll_vel.abs() < 0.1 { self.scroll_vel = 0.0; }
    }

    /// Kick the scroll velocity based on a pull gesture.
    /// `velocity` is normalised 0.0–1.0.
    pub fn kick(&mut self, velocity: f32) {
        self.scroll_vel = (velocity * 12.0).min(18.0);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// StitchState — the animated connection between ribbons when playing
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq)]
pub enum StitchPhase {
    /// Ribbons are separate.
    Unstitched,
    /// Ribbons are animating toward each other (clap in progress).
    Stitching { progress: f32 },
    /// Ribbons fully stitched — MIDI playing.
    Stitched,
    /// Ribbons separating (unclap).
    Unstitching { progress: f32 },
}

impl StitchPhase {
    pub fn is_stitched(&self) -> bool {
        matches!(self, StitchPhase::Stitched | StitchPhase::Stitching { .. })
    }

    /// Advance one frame.  Returns true when transition completes.
    pub fn tick(&mut self) -> bool {
        match self {
            StitchPhase::Stitching { progress } => {
                *progress += 0.05;
                if *progress >= 1.0 {
                    *self = StitchPhase::Stitched;
                    return true;
                }
            }
            StitchPhase::Unstitching { progress } => {
                *progress += 0.05;
                if *progress >= 1.0 {
                    *self = StitchPhase::Unstitched;
                    return true;
                }
            }
            _ => {}
        }
        false
    }
}

// ════════════════════════════════════════════════════════════════════════════
// SnippetTray — deposited snippets shown on the right side of the screen
// ════════════════════════════════════════════════════════════════════════════

/// A snippet deposited into the tray.
#[derive(Clone, Debug)]
pub struct TrayEntry {
    pub name:    String,
    pub patches: Vec<(Patch, Patch)>,  // (left_patch, right_patch) pairs
    /// Animation: how far the entry has slid into the tray (0.0–1.0).
    pub slide_in: f32,
}

/// The on-screen snippet tray on the right side of the window.
#[derive(Debug, Default)]
pub struct SnippetTray {
    pub entries: Vec<TrayEntry>,
}

impl SnippetTray {
    pub fn deposit(&mut self, name: &str, pairs: Vec<(Patch, Patch)>) {
        self.entries.push(TrayEntry {
            name:     name.to_string(),
            patches:  pairs,
            slide_in: 0.0,
        });
        // Keep at most 8 entries visible
        if self.entries.len() > 8 {
            self.entries.remove(0);
        }
    }

    /// Advance slide-in animations.
    pub fn tick(&mut self) {
        for e in &mut self.entries {
            if e.slide_in < 1.0 {
                e.slide_in = (e.slide_in + 0.08).min(1.0);
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ScissorAnimation — visual highlight during snip
// ════════════════════════════════════════════════════════════════════════════

/// Overlay drawn on top of the stitched ribbon section during a snip.
#[derive(Clone, Debug)]
pub struct ScissorAnimation {
    /// Progress 0.0–1.0; drives the gold highlight sweep.
    pub progress: f32,
    /// The patch range being snipped (left index, count).
    pub start_patch: usize,
    pub count:       usize,
}

impl ScissorAnimation {
    pub fn new(start_patch: usize, count: usize) -> Self {
        ScissorAnimation { progress: 0.0, start_patch, count }
    }
    pub fn tick(&mut self) { self.progress = (self.progress + 0.04).min(1.0); }
    pub fn done(&self) -> bool { self.progress >= 1.0 }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_color_distinct() {
        // Different digits should produce different colors
        let c0 = digit_color(0, 10);
        let c5 = digit_color(5, 10);
        assert_ne!(c0, c5);
    }

    #[test]
    fn digit_color_alpha_opaque() {
        for d in 0..10 {
            let c = digit_color(d, 10);
            assert_eq!(c >> 24, 0xFF, "digit {} color should be opaque", d);
        }
    }

    #[test]
    fn ribbon_capacity() {
        let mut r = RibbonState::new(5, 10, "test");
        for i in 0..8 { r.push(i % 10, i); }
        assert_eq!(r.patches.len(), 5);
        assert_eq!(r.patches.last().unwrap().digit, 7);
    }

    #[test]
    fn ribbon_scroll_friction() {
        let mut r = RibbonState::new(10, 10, "test");
        r.kick(1.0);
        assert!(r.scroll_vel > 0.0);
        for _ in 0..100 { r.tick(40.0); }
        assert_eq!(r.scroll_vel, 0.0);
    }

    #[test]
    fn stitch_phase_stitching_completes() {
        let mut p = StitchPhase::Stitching { progress: 0.0 };
        let mut done = false;
        for _ in 0..100 {
            if p.tick() { done = true; break; }
        }
        assert!(done);
        assert_eq!(p, StitchPhase::Stitched);
    }

    #[test]
    fn tray_max_entries() {
        let mut tray = SnippetTray::default();
        for i in 0..10 {
            tray.deposit(&format!("s{}", i), vec![]);
        }
        assert!(tray.entries.len() <= 8);
    }
}
