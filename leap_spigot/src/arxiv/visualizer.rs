//! Software-rendered visualizer using `minifb`.
//!
//! Layout:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┬──────────────┐
//! │  [LEFT RIBBON  — duration stream ──────────────────]│  SNIPPET     │
//! │                                                     │  TRAY        │
//! │  [stitch threads when playing]                      │              │
//! │                                                     │  [entry 0]   │
//! │  [RIGHT RIBBON — pitch stream ─────────────────────]│  [entry 1]   │
//! │                                                     │  [entry 2]   │
//! │  status bar                                         │              │
//! └─────────────────────────────────────────────────────┴──────────────┘
//! ```

use minifb::{Key, Window, WindowOptions, KeyRepeat};
use crate::gesture::{SimInput, SimKey};
use crate::ribbon::{
    RibbonState, StitchPhase, SnippetTray, ScissorAnimation,
};

use std::sync::mpsc::Sender;

// ════════════════════════════════════════════════════════════════════════════
// Layout constants
// ════════════════════════════════════════════════════════════════════════════

pub const WIN_W:       usize = 1200;
pub const WIN_H:       usize = 500;
const TRAY_W:          usize = 220;
const RIBBON_W:        usize = WIN_W - TRAY_W;
const RIBBON_H:        usize = 90;
const PATCH_W:         usize = 48;
const PATCH_H:         usize = RIBBON_H;
const LEFT_RIBBON_Y:   usize = 60;
const RIGHT_RIBBON_Y:  usize = 310;
const STATUS_Y:        usize = WIN_H - 36;
const BG_COLOR:        u32   = 0xFF1A1A2E;
const TRAY_BG:         u32   = 0xFF16213E;
const STITCH_COLOR:    u32   = 0xFFFFD700;  // gold
const HIGHLIGHT_COLOR: u32   = 0xFFFFFF00;  // scissors highlight
const TEXT_BG:         u32   = 0xFF0F3460;

// ════════════════════════════════════════════════════════════════════════════
// Visualizer
// ════════════════════════════════════════════════════════════════════════════

pub struct Visualizer {
    window:   Window,
    buf:      Vec<u32>,
    sim_tx:   Sender<SimInput>,

    // State references (owned here, shared to app via Arc<Mutex> in real use;
    // here we take snapshots each frame passed from AppState).
}

impl Visualizer {
    pub fn new(sim_tx: Sender<SimInput>) -> Result<Self, String> {
        let mut window = Window::new(
            "Leap Spigot — Transcendental MIDI Ribbon",
            WIN_W, WIN_H,
            WindowOptions {
                resize: false,
                ..WindowOptions::default()
            },
        ).map_err(|e| e.to_string())?;

        window.limit_update_rate(Some(std::time::Duration::from_millis(16))); // ~60fps

        Ok(Visualizer {
            window,
            buf: vec![BG_COLOR; WIN_W * WIN_H],
            sim_tx,
        })
    }

    /// Returns false when the window should close.
    pub fn is_open(&self) -> bool { self.window.is_open() }

    /// Poll keyboard inputs and translate to SimInput events.
    pub fn poll_input(&mut self) -> bool {
        if !self.window.is_open() { return false; }

        let shift = self.window.is_key_down(Key::LeftShift)
                 || self.window.is_key_down(Key::RightShift);

        // Keys that trigger on first press only
        let one_shot = |k: Key| self.window.is_key_pressed(k, KeyRepeat::No);
        // Keys that repeat while held
        let held     = |k: Key| self.window.is_key_pressed(k, KeyRepeat::Yes);

        if one_shot(Key::Q) {
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Quit));
            return false;
        }
        if one_shot(Key::T) {
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Twist));
        }
        if one_shot(Key::Space) {
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Clap));
        }
        if one_shot(Key::Escape) {
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Unclap));
        }
        if one_shot(Key::S) {
            // Scissors: prompt for name, then send
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Scissors));
        }

        // Pull left (A — repeats for held advance)
        if held(Key::A) {
            let key = if shift { SimKey::PullLeftFast } else { SimKey::PullLeft };
            let _ = self.sim_tx.send(SimInput::KeyDown(key));
        }
        // Pull right (D)
        if held(Key::D) {
            let key = if shift { SimKey::PullRightFast } else { SimKey::PullRight };
            let _ = self.sim_tx.send(SimInput::KeyDown(key));
        }

        true
    }

    /// Render one frame.
    pub fn render(
        &mut self,
        left:     &RibbonState,
        right:    &RibbonState,
        stitch:   &StitchPhase,
        tray:     &SnippetTray,
        scissor:  &Option<ScissorAnimation>,
        status:   &str,
        playing:  bool,
        note_highlight: Option<usize>,   // left-ribbon patch index of current note
    ) {
        // Clear
        self.buf.fill(BG_COLOR);

        // ── Tray background ───────────────────────────────────────────────
        self.fill_rect(RIBBON_W, 0, TRAY_W, WIN_H, TRAY_BG);

        // ── Draw ribbons ──────────────────────────────────────────────────
        self.draw_ribbon(left,  LEFT_RIBBON_Y,  note_highlight, false);
        self.draw_ribbon(right, RIGHT_RIBBON_Y, None, false);

        // ── Ribbon labels ─────────────────────────────────────────────────
        self.draw_label(&left.label,  8, LEFT_RIBBON_Y  - 22, 0xFFAADDFF);
        self.draw_label(&right.label, 8, RIGHT_RIBBON_Y - 22, 0xFFFFBBAA);

        // ── Stitch threads ────────────────────────────────────────────────
        if stitch.is_stitched() {
            let prog = match stitch {
                StitchPhase::Stitching   { progress } => *progress,
                StitchPhase::Stitched                 => 1.0,
                StitchPhase::Unstitching { progress } => 1.0 - progress,
                _ => 0.0,
            };
            self.draw_stitch_threads(prog);
        }

        // ── Scissor highlight overlay ─────────────────────────────────────
        if let Some(sc) = scissor {
            self.draw_scissor_highlight(sc);
        }

        // ── Playing pulse on patch borders ────────────────────────────────
        if playing {
            self.draw_playing_border(LEFT_RIBBON_Y);
            self.draw_playing_border(RIGHT_RIBBON_Y);
        }

        // ── Snippet tray ──────────────────────────────────────────────────
        self.draw_tray(tray);

        // ── Status bar ────────────────────────────────────────────────────
        self.fill_rect(0, STATUS_Y, RIBBON_W, WIN_H - STATUS_Y, TEXT_BG);
        self.draw_label(status, 10, STATUS_Y + 10, 0xFFEEEEEE);

        // ── Key legend ────────────────────────────────────────────────────
        self.draw_label(
            "A/D=pull  Shift+A/D=fast  T=twist  Space=clap  Esc=unclap  S=snip  Q=quit",
            10, WIN_H - 16, 0xFF888888,
        );

        self.window.update_with_buffer(&self.buf, WIN_W, WIN_H).ok();
    }

    // ── Ribbon ────────────────────────────────────────────────────────────

    fn draw_ribbon(
        &mut self,
        ribbon: &RibbonState,
        y: usize,
        highlight_idx: Option<usize>,
        _mirror: bool,
    ) {
        let scroll = ribbon.scroll_px as isize;

        for (i, patch) in ribbon.patches.iter().enumerate() {
            let px = (i * PATCH_W) as isize - scroll;
            if px + PATCH_W as isize <= 0 { continue; }
            if px >= RIBBON_W as isize    { break;    }

            let x0 = px.max(0) as usize;
            let x1 = (px + PATCH_W as isize).min(RIBBON_W as isize) as usize;

            // Slightly brighten highlighted (currently playing) patch
            let color = if highlight_idx == Some(i) {
                blend(patch.color, 0xFFFFFFFF, 0.35)
            } else {
                patch.color
            };

            self.fill_rect(x0, y, x1 - x0, PATCH_H, color);

            // Digit label in centre of patch
            let lx = x0 + (x1 - x0).saturating_sub(6) / 2;
            let ly = y + PATCH_H / 2 - 4;
            let digit_str = format!("{}", patch.digit);
            self.draw_label(&digit_str, lx, ly, 0xFF000000);

            // Border
            self.draw_border(x0, y, x1 - x0, PATCH_H, 0xFF000000);
        }
    }

    // ── Stitch threads ────────────────────────────────────────────────────

    fn draw_stitch_threads(&mut self, progress: f32) {
        let y_top    = LEFT_RIBBON_Y  + PATCH_H;
        let y_bottom = RIGHT_RIBBON_Y;
        let mid_y    = (y_top + y_bottom) / 2;
        let visible  = (RIBBON_W / PATCH_W).min(20);

        for i in 0..visible {
            let cx = i * PATCH_W + PATCH_W / 2;
            // The thread "grows" from top downward as progress → 1.0
            let thread_bottom = y_top + ((y_bottom - y_top) as f32 * progress) as usize;

            // Vertical thread
            for y in y_top..thread_bottom {
                self.set_pixel(cx,     y, STITCH_COLOR);
                self.set_pixel(cx + 1, y, STITCH_COLOR);
            }
            // Diamond knot at mid-point when fully stitched
            if progress > 0.9 {
                self.draw_diamond(cx, mid_y, 4, STITCH_COLOR);
            }
        }
    }

    // ── Scissor highlight ─────────────────────────────────────────────────

    fn draw_scissor_highlight(&mut self, sc: &ScissorAnimation) {
        let end = (sc.start_patch + (sc.count as f32 * sc.progress) as usize)
            .min(sc.start_patch + sc.count);

        for i in sc.start_patch..end {
            let x0 = i * PATCH_W;
            if x0 >= RIBBON_W { break; }
            let w = PATCH_W.min(RIBBON_W - x0);
            self.draw_border(x0, LEFT_RIBBON_Y,  w, PATCH_H, HIGHLIGHT_COLOR);
            self.draw_border(x0, RIGHT_RIBBON_Y, w, PATCH_H, HIGHLIGHT_COLOR);
        }
    }

    // ── Playing border pulse ──────────────────────────────────────────────

    fn draw_playing_border(&mut self, y: usize) {
        self.draw_border(0, y, RIBBON_W, PATCH_H, STITCH_COLOR);
    }

    // ── Snippet tray ──────────────────────────────────────────────────────

    fn draw_tray(&mut self, tray: &SnippetTray) {
        self.draw_label("SNIPPET TRAY", RIBBON_W + 10, 10, 0xFFFFD700);

        let mut ey = 40usize;
        for entry in &tray.entries {
            let slide = entry.slide_in;
            let ex = RIBBON_W + (TRAY_W as f32 * (1.0 - slide)) as usize;

            // Entry background
            if ex < WIN_W {
                self.fill_rect(ex, ey, WIN_W - ex, 52, 0xFF0F3460);
                self.draw_label(&entry.name, ex + 6, ey + 4, 0xFFFFD700);

                // Mini ribbon strip
                let max_patches = 8;
                let pw = (TRAY_W - 20) / max_patches;
                for (j, (lp, rp)) in entry.patches.iter().take(max_patches).enumerate() {
                    let px = ex + 6 + j * pw;
                    let ph = 16;
                    self.fill_rect(px, ey + 18, pw.saturating_sub(2), ph, lp.color);
                    self.fill_rect(px, ey + 36, pw.saturating_sub(2), ph, rp.color);
                }
            }
            ey += 58;
            if ey + 58 > STATUS_Y { break; }
        }
    }

    // ── Primitive drawing helpers ─────────────────────────────────────────

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for row in y..(y+h).min(WIN_H) {
            for col in x..(x+w).min(WIN_W) {
                self.buf[row * WIN_W + col] = color;
            }
        }
    }

    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for col in x..(x+w).min(WIN_W) {
            if y < WIN_H           { self.buf[y           * WIN_W + col] = color; }
            if y+h-1 < WIN_H       { self.buf[(y+h-1)     * WIN_W + col] = color; }
        }
        for row in y..(y+h).min(WIN_H) {
            if x < WIN_W           { self.buf[row * WIN_W + x    ] = color; }
            if x+w-1 < WIN_W       { self.buf[row * WIN_W + x+w-1] = color; }
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < WIN_W && y < WIN_H {
            self.buf[y * WIN_W + x] = color;
        }
    }

    fn draw_diamond(&mut self, cx: usize, cy: usize, r: usize, color: u32) {
        for dy in 0..=r as isize {
            let dx = r as isize - dy;
            for &(sx, sy) in &[
                (cx as isize + dx, cy as isize + dy),
                (cx as isize - dx, cy as isize + dy),
                (cx as isize + dx, cy as isize - dy),
                (cx as isize - dx, cy as isize - dy),
            ] {
                if sx >= 0 && sy >= 0 {
                    self.set_pixel(sx as usize, sy as usize, color);
                }
            }
        }
    }

    /// Minimal bitmap font — 3×5 characters for digit/label rendering.
    /// Each character is encoded as 5 rows × 3 bits.
    fn draw_label(&mut self, text: &str, x: usize, y: usize, color: u32) {
        let mut cx = x;
        for ch in text.chars() {
            let glyph = char_glyph(ch);
            for (row, &bits) in glyph.iter().enumerate() {
                for col in 0..3usize {
                    if bits & (1 << (2 - col)) != 0 {
                        self.set_pixel(cx + col, y + row, color);
                    }
                }
            }
            cx += 4; // 3 wide + 1 gap
            if cx + 4 > WIN_W { break; }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Minimal 3×5 bitmap font
// ────────────────────────────────────────────────────────────────────────────

fn char_glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'a' | 'A' => [0b111, 0b101, 0b111, 0b101, 0b101],
        'b' | 'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'c' | 'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'd' | 'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'e' | 'E' => [0b111, 0b100, 0b111, 0b100, 0b111],
        'f' | 'F' => [0b111, 0b100, 0b111, 0b100, 0b100],
        'g' | 'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'h' | 'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'i' | 'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'j' | 'J' => [0b001, 0b001, 0b001, 0b101, 0b111],
        'k' | 'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'l' | 'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'm' | 'M' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'n' | 'N' => [0b111, 0b101, 0b101, 0b101, 0b101],
        'o' | 'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'p' | 'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'r' | 'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        's' | 'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        't' | 'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'u' | 'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'v' | 'V' => [0b101, 0b101, 0b101, 0b010, 0b010],
        'w' | 'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'x' | 'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'y' | 'Y' => [0b101, 0b101, 0b111, 0b010, 0b010],
        'z' | 'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        _   => [0b000, 0b000, 0b010, 0b000, 0b000], // fallback dot
    }
}

/// Alpha-blend two ARGB colors. `t` = 0.0 → all `a`, `t` = 1.0 → all `b`.
fn blend(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |ca: u32, cb: u32| (ca as f32 * (1.0-t) + cb as f32 * t) as u32;
    let ar = (a >> 16) & 0xFF; let br = (b >> 16) & 0xFF;
    let ag = (a >>  8) & 0xFF; let bg = (b >>  8) & 0xFF;
    let ab =  a        & 0xFF; let bb =  b        & 0xFF;
    0xFF000000 | (lerp(ar,br) << 16) | (lerp(ag,bg) << 8) | lerp(ab,bb)
}
