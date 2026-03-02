//! Software-rendered visualizer using `minifb`.
//!
//! Three layout modes selected at startup via `--layout`:
//!
//! **flat** (default) — horizontal ribbons, classic left-to-right view.
//!
//! ```text
//! ┌──────────────────────────────────────────┬──────────┐
//! │  [LEFT RIBBON  ─────────────────────────]│  SNIPPET │
//! │  [stitch threads when playing]           │  TRAY    │
//! │  [RIGHT RIBBON ─────────────────────────]│          │
//! │  status bar                              │          │
//! └──────────────────────────────────────────┴──────────┘
//! ```
//!
//! **2d** — vertical ribbons rising from the bottom of the screen.
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │   LEFT label          RIGHT label                  │
//! │     │                   │                          │
//! │  ┌──┴──┐             ┌──┴──┐                       │
//! │  │     │  ─stitch──  │     │                       │
//! │  │patch│             │patch│   SNIPPET TRAY        │
//! │  │  ·  │             │  ·  │                       │
//! │  │patch│             │patch│                       │
//! │  └─────┘             └─────┘                       │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! **3d** — perspective view with ribbons receding toward a vanishing point,
//!          plus wireframe hand ghosts giving gesture feedback.
//!
//! ```text
//!  LEFT ──────────────────────────────────── vanishing pt
//!      □ □ □ □ □ □ □ □ □ · · · ·
//!                                           [left hand]
//!                                           [right hand]
//!      □ □ □ □ □ □ □ □ □ · · · ·
//!  RIGHT ─────────────────────────────────── vanishing pt
//! ```

use minifb::{Key, Window, WindowOptions, KeyRepeat};
use crate::gesture::{SimInput, SimKey, GestureEvent};
use crate::ribbon::{
    RibbonState, StitchPhase, SnippetTray, ScissorAnimation,
};
use std::sync::mpsc::Sender;

// ════════════════════════════════════════════════════════════════════════════
// LayoutMode
// ════════════════════════════════════════════════════════════════════════════

/// Which visual layout to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// Classic horizontal ribbons (original).
    Flat,
    /// Vertical ribbons rising from the bottom.
    TwoD,
    /// Perspective ribbons receding into the screen with hand ghosts.
    ThreeD,
}

impl LayoutMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "2d"   => LayoutMode::TwoD,
            "3d"   => LayoutMode::ThreeD,
            _      => LayoutMode::Flat,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Window / buffer constants
// ════════════════════════════════════════════════════════════════════════════

pub const WIN_W: usize = 1280;
pub const WIN_H: usize = 720;

const BG_COLOR:        u32 = 0xFF1A1A2E;
const TRAY_BG:         u32 = 0xFF16213E;
const STITCH_COLOR:    u32 = 0xFFFFD700;
const HIGHLIGHT_COLOR: u32 = 0xFFFFFF00;
const TEXT_BG:         u32 = 0xFF0F3460;
const TRAY_W:          usize = 220;

// ── Flat layout ────────────────────────────────────────────────────────────
const FLAT_RIBBON_W:   usize = WIN_W - TRAY_W;
const FLAT_PATCH_W:    usize = 48;
const FLAT_PATCH_H:    usize = 90;
const FLAT_LEFT_Y:     usize = 60;
const FLAT_RIGHT_Y:    usize = 340;
const FLAT_STATUS_Y:   usize = WIN_H - 36;

// ── 2D layout ──────────────────────────────────────────────────────────────
const TD_PATCH_W:      usize = 80;
const TD_PATCH_H:      usize = 48;
const TD_LEFT_X:       usize = 120;
const TD_RIGHT_X:      usize = 520;
const TD_RIBBON_W:     usize = TD_PATCH_W;
const TD_BOTTOM_Y:     usize = WIN_H - 80;

// ── 3D layout ──────────────────────────────────────────────────────────────
const P3_VPX:          f32   = WIN_W as f32 / 2.0;   // vanishing point x
const P3_VPY:          f32   = WIN_H as f32 / 2.0;   // vanishing point y
const P3_FOCAL:        f32   = 600.0;                 // focal length
const P3_NEAR_Z:       f32   = 0.5;                   // nearest patch z
const P3_FAR_Z:        f32   = 20.0;                  // farthest patch z
const P3_LEFT_WORLD_Y: f32   = -1.4;                  // world-Y of left ribbon
const P3_RIGHT_WORLD_Y:f32   =  1.4;                  // world-Y of right ribbon
const P3_PATCH_DEPTH:  f32   = 0.9;                   // z-spacing between patches
const P3_PATCH_HALF_W: f32   = 0.55;                  // half-width of patch in world units

// ════════════════════════════════════════════════════════════════════════════
// GestureState — tracked for hand ghost animation
// ════════════════════════════════════════════════════════════════════════════

/// Simplified gesture state for the hand ghost renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandGesture {
    Idle,
    PullLeft,
    PullRight,
    Twist,
    Clap,
    Scissors,
}

// ════════════════════════════════════════════════════════════════════════════
// Visualizer
// ════════════════════════════════════════════════════════════════════════════

pub struct Visualizer {
    window:        Window,
    buf:           Vec<u32>,
    sim_tx:        Sender<SimInput>,
    pub layout:    LayoutMode,
    /// Last known gesture for hand ghost animation.
    hand_gesture:  HandGesture,
    /// Frame counter — drives subtle animations.
    frame:         u64,
}

impl Visualizer {
    pub fn new(sim_tx: Sender<SimInput>, layout: LayoutMode) -> Result<Self, String> {
        let title = match layout {
            LayoutMode::Flat   => "Leap Spigot — Flat View",
            LayoutMode::TwoD   => "Leap Spigot — 2D View",
            LayoutMode::ThreeD => "Leap Spigot — 3D View",
        };
        let mut window = Window::new(
            title, WIN_W, WIN_H,
            WindowOptions { resize: false, ..WindowOptions::default() },
        ).map_err(|e| e.to_string())?;

        window.set_target_fps(60);

        Ok(Visualizer {
            window,
            buf: vec![BG_COLOR; WIN_W * WIN_H],
            sim_tx,
            layout,
            hand_gesture: HandGesture::Idle,
            frame: 0,
        })
    }

    pub fn is_open(&self) -> bool { self.window.is_open() }

    /// Note the most recent gesture so hand ghosts can animate.
    pub fn notify_gesture(&mut self, g: HandGesture) {
        self.hand_gesture = g;
    }

    // ── input polling ─────────────────────────────────────────────────────

    pub fn poll_input(&mut self) -> bool {
        if !self.window.is_open() { return false; }

        let shift = self.window.is_key_down(Key::LeftShift)
                 || self.window.is_key_down(Key::RightShift);
        let one_shot = |k: Key| self.window.is_key_pressed(k, KeyRepeat::No);
        let held     = |k: Key| self.window.is_key_pressed(k, KeyRepeat::Yes);

        if one_shot(Key::Q) {
            let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Quit));
            return false;
        }
        if one_shot(Key::T) { let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Twist)); }
        if one_shot(Key::Space)  { let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Clap)); }
        if one_shot(Key::Escape) { let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Unclap)); }
        if one_shot(Key::S)      { let _ = self.sim_tx.send(SimInput::KeyDown(SimKey::Scissors)); }

        if held(Key::A) {
            let k = if shift { SimKey::PullLeftFast } else { SimKey::PullLeft };
            let _ = self.sim_tx.send(SimInput::KeyDown(k));
        }
        if held(Key::D) {
            let k = if shift { SimKey::PullRightFast } else { SimKey::PullRight };
            let _ = self.sim_tx.send(SimInput::KeyDown(k));
        }
        true
    }

    // ── master render dispatch ────────────────────────────────────────────

    pub fn render(
        &mut self,
        left:           &RibbonState,
        right:          &RibbonState,
        stitch:         &StitchPhase,
        tray:           &SnippetTray,
        scissor:        &Option<ScissorAnimation>,
        status:         &str,
        playing:        bool,
        note_highlight: Option<usize>,
    ) {
        self.buf.fill(BG_COLOR);
        self.frame = self.frame.wrapping_add(1);

        match self.layout {
            LayoutMode::Flat   => self.render_flat(left, right, stitch, tray, scissor,
                                                    status, playing, note_highlight),
            LayoutMode::TwoD   => self.render_2d(left, right, stitch, tray, scissor,
                                                  status, playing, note_highlight),
            LayoutMode::ThreeD => self.render_3d(left, right, stitch, tray, scissor,
                                                  status, playing, note_highlight),
        }

        // Status bar and legend are common to all modes
        let legend_y = WIN_H - 16;
        self.fill_rect(0, WIN_H - 36, WIN_W, 36, TEXT_BG);
        self.draw_label(status, 10, WIN_H - 30, 0xFFEEEEEE);
        self.draw_label(
            "A/D=pull  Shift+A/D=fast  T=twist  Space=clap  Esc=unclap  S=snip  Q=quit",
            10, legend_y, 0xFF888888,
        );

        self.window.update_with_buffer(&self.buf, WIN_W, WIN_H).ok();
    }

    // ════════════════════════════════════════════════════════════════════════
    // FLAT layout renderer
    // ════════════════════════════════════════════════════════════════════════

    fn render_flat(
        &mut self,
        left:           &RibbonState,
        right:          &RibbonState,
        stitch:         &StitchPhase,
        tray:           &SnippetTray,
        scissor:        &Option<ScissorAnimation>,
        _status:        &str,
        playing:        bool,
        note_highlight: Option<usize>,
    ) {
        self.fill_rect(FLAT_RIBBON_W, 0, TRAY_W, WIN_H, TRAY_BG);

        self.draw_ribbon_flat(left,  FLAT_LEFT_Y,  note_highlight);
        self.draw_ribbon_flat(right, FLAT_RIGHT_Y, None);

        self.draw_label(&left.label,  8, FLAT_LEFT_Y  - 22, 0xFFAADDFF);
        self.draw_label(&right.label, 8, FLAT_RIGHT_Y - 22, 0xFFFFBBAA);

        if stitch.is_stitched() {
            let prog = stitch_progress(stitch);
            self.draw_flat_stitch(prog);
        }
        if let Some(sc) = scissor { self.draw_flat_scissor(sc); }
        if playing {
            self.draw_border(0, FLAT_LEFT_Y,  FLAT_RIBBON_W, FLAT_PATCH_H, STITCH_COLOR);
            self.draw_border(0, FLAT_RIGHT_Y, FLAT_RIBBON_W, FLAT_PATCH_H, STITCH_COLOR);
        }
        self.draw_tray(tray, FLAT_RIBBON_W);
    }

    fn draw_ribbon_flat(&mut self, ribbon: &RibbonState, y: usize, highlight: Option<usize>) {
        let scroll = ribbon.scroll_px as isize;
        for (i, patch) in ribbon.patches.iter().enumerate() {
            let px = (i * FLAT_PATCH_W) as isize - scroll;
            if px + FLAT_PATCH_W as isize <= 0 { continue; }
            if px >= FLAT_RIBBON_W as isize     { break;    }
            let x0 = px.max(0) as usize;
            let x1 = (px + FLAT_PATCH_W as isize).min(FLAT_RIBBON_W as isize) as usize;
            let color = if highlight == Some(i) { blend(patch.color, 0xFFFFFFFF, 0.35) }
                        else { patch.color };
            self.fill_rect(x0, y, x1 - x0, FLAT_PATCH_H, color);
            let lx = x0 + (x1 - x0).saturating_sub(6) / 2;
            self.draw_label(&format!("{}", patch.digit), lx, y + FLAT_PATCH_H/2 - 4, 0xFF000000);
            self.draw_border(x0, y, x1 - x0, FLAT_PATCH_H, 0xFF000000);
        }
    }

    fn draw_flat_stitch(&mut self, progress: f32) {
        let y_top    = FLAT_LEFT_Y  + FLAT_PATCH_H;
        let y_bottom = FLAT_RIGHT_Y;
        let mid_y    = (y_top + y_bottom) / 2;
        let visible  = FLAT_RIBBON_W / FLAT_PATCH_W;
        for i in 0..visible {
            let cx = i * FLAT_PATCH_W + FLAT_PATCH_W / 2;
            let thread_bottom = y_top + ((y_bottom - y_top) as f32 * progress) as usize;
            for y in y_top..thread_bottom {
                self.set_pixel(cx,     y, STITCH_COLOR);
                self.set_pixel(cx + 1, y, STITCH_COLOR);
            }
            if progress > 0.9 { self.draw_diamond(cx, mid_y, 4, STITCH_COLOR); }
        }
    }

    fn draw_flat_scissor(&mut self, sc: &ScissorAnimation) {
        let end = sc.start_patch + (sc.count as f32 * sc.progress) as usize;
        for i in sc.start_patch..end {
            let x0 = i * FLAT_PATCH_W;
            if x0 >= FLAT_RIBBON_W { break; }
            let w = FLAT_PATCH_W.min(FLAT_RIBBON_W - x0);
            self.draw_border(x0, FLAT_LEFT_Y,  w, FLAT_PATCH_H, HIGHLIGHT_COLOR);
            self.draw_border(x0, FLAT_RIGHT_Y, w, FLAT_PATCH_H, HIGHLIGHT_COLOR);
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2D layout renderer  — vertical columns from the bottom
    // ════════════════════════════════════════════════════════════════════════

    fn render_2d(
        &mut self,
        left:           &RibbonState,
        right:          &RibbonState,
        stitch:         &StitchPhase,
        tray:           &SnippetTray,
        scissor:        &Option<ScissorAnimation>,
        _status:        &str,
        playing:        bool,
        note_highlight: Option<usize>,
    ) {
        // Tray on the right
        let content_w = WIN_W - TRAY_W;
        self.fill_rect(content_w, 0, TRAY_W, WIN_H, TRAY_BG);

        self.draw_ribbon_2d(left,  TD_LEFT_X,  note_highlight, 0xFFAADDFF);
        self.draw_ribbon_2d(right, TD_RIGHT_X, None,           0xFFFFBBAA);

        // Labels at top of columns
        self.draw_label(&left.label,  TD_LEFT_X,  30, 0xFFAADDFF);
        self.draw_label(&right.label, TD_RIGHT_X, 30, 0xFFFFBBAA);

        // Stitch threads: horizontal lines connecting the two columns
        if stitch.is_stitched() {
            let prog   = stitch_progress(stitch);
            let mid_x1 = TD_LEFT_X  + TD_RIBBON_W;
            let mid_x2 = TD_RIGHT_X;
            let visible  = (TD_BOTTOM_Y - 60) / TD_PATCH_H;
            for i in 0..((visible as f32 * prog) as usize) {
                let patch_y = TD_BOTTOM_Y.saturating_sub(i * TD_PATCH_H + TD_PATCH_H / 2);
                for x in mid_x1..mid_x2 {
                    self.set_pixel(x, patch_y, STITCH_COLOR);
                    self.set_pixel(x, patch_y + 1, STITCH_COLOR);
                }
                self.draw_diamond((mid_x1 + mid_x2) / 2, patch_y, 4, STITCH_COLOR);
            }
        }

        // Scissor highlight: horizontal gold bars
        if let Some(sc) = scissor {
            let end = sc.start_patch + (sc.count as f32 * sc.progress) as usize;
            for i in sc.start_patch..end {
                let py = TD_BOTTOM_Y.saturating_sub(i * TD_PATCH_H);
                self.draw_border(TD_LEFT_X,  py, TD_RIBBON_W, TD_PATCH_H, HIGHLIGHT_COLOR);
                self.draw_border(TD_RIGHT_X, py, TD_RIBBON_W, TD_PATCH_H, HIGHLIGHT_COLOR);
            }
        }

        // Playing pulse
        if playing {
            self.draw_border(TD_LEFT_X,  0, TD_RIBBON_W, WIN_H, STITCH_COLOR);
            self.draw_border(TD_RIGHT_X, 0, TD_RIBBON_W, WIN_H, STITCH_COLOR);
        }

        self.draw_tray(tray, content_w);
    }

    fn draw_ribbon_2d(
        &mut self,
        ribbon: &RibbonState,
        x: usize,
        highlight: Option<usize>,
        label_color: u32,
    ) {
        let scroll = ribbon.scroll_px as isize;
        for (i, patch) in ribbon.patches.iter().enumerate() {
            // Patches stack upward from bottom; newest = bottommost
            let raw_py = TD_BOTTOM_Y as isize - (i as isize + 1) * TD_PATCH_H as isize
                       + scroll;
            if raw_py + TD_PATCH_H as isize <= 0 { break; }
            if raw_py >= WIN_H as isize           { continue; }
            let py = raw_py.max(0) as usize;
            let ph = TD_PATCH_H.min(WIN_H - py);

            let color = if highlight == Some(i) { blend(patch.color, 0xFFFFFFFF, 0.35) }
                        else { patch.color };
            self.fill_rect(x, py, TD_RIBBON_W, ph, color);
            self.draw_label(&format!("{}", patch.digit), x + 4, py + ph/2 - 2, 0xFF000000);
            self.draw_border(x, py, TD_RIBBON_W, ph, 0xFF000000);
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // 3D layout renderer  — perspective + hand ghosts
    // ════════════════════════════════════════════════════════════════════════

    fn render_3d(
        &mut self,
        left:           &RibbonState,
        right:          &RibbonState,
        stitch:         &StitchPhase,
        tray:           &SnippetTray,
        scissor:        &Option<ScissorAnimation>,
        _status:        &str,
        playing:        bool,
        note_highlight: Option<usize>,
    ) {
        // Sky gradient — deep space feel
        self.draw_vertical_gradient(0, WIN_H, 0xFF0A0A1A, 0xFF1A1A3E);

        // Vanishing-point grid lines
        self.draw_vp_grid();

        // Ribbons receding into the screen
        self.draw_ribbon_3d(left,  P3_LEFT_WORLD_Y,  note_highlight, 0xFFAADDFF);
        self.draw_ribbon_3d(right, P3_RIGHT_WORLD_Y, None,           0xFFFFBBAA);

        // Stitch threads — arcs/lines in 3D connecting the two ribbons
        if stitch.is_stitched() {
            let prog = stitch_progress(stitch);
            self.draw_3d_stitch(prog);
        }

        // Scissor highlight
        if let Some(sc) = scissor {
            self.draw_3d_scissor(sc);
        }

        // Playing frame glow
        if playing {
            let glow = pulse_alpha(self.frame, 0.4, 0.9);
            let c    = blend(BG_COLOR, STITCH_COLOR, glow);
            self.draw_border(2, 2, WIN_W - 4, WIN_H - 40, c);
            self.draw_border(4, 4, WIN_W - 8, WIN_H - 44, c);
        }

        // Hand ghosts — always drawn in 3D mode
        self.draw_hand_ghosts();

        // Tray (right side, semi-transparent feel)
        let content_w = WIN_W - TRAY_W;
        self.fill_rect(content_w, 0, TRAY_W, WIN_H - 36, TRAY_BG);
        self.draw_tray(tray, content_w);

        // Labels near the near edge of each ribbon
        let (lsx, lsy) = project_3d(P3_PATCH_HALF_W * 2.0, P3_LEFT_WORLD_Y, P3_NEAR_Z);
        let (rsx, rsy) = project_3d(P3_PATCH_HALF_W * 2.0, P3_RIGHT_WORLD_Y, P3_NEAR_Z);
        self.draw_label(&left.label,  lsx as usize, lsy as usize + 8, 0xFFAADDFF);
        self.draw_label(&right.label, rsx as usize, rsy as usize + 8, 0xFFFFBBAA);
    }

    fn draw_ribbon_3d(
        &mut self,
        ribbon:    &RibbonState,
        world_y:   f32,
        highlight: Option<usize>,
        _tint:     u32,
    ) {
        // Patches are laid out along the Z axis; patch 0 = nearest (z=NEAR_Z),
        // each subsequent patch is P3_PATCH_DEPTH units farther away.
        let n = ribbon.patches.len();
        if n == 0 { return; }

        for (i, patch) in ribbon.patches.iter().enumerate() {
            let z = P3_NEAR_Z + i as f32 * P3_PATCH_DEPTH;
            if z > P3_FAR_Z { break; }

            // Perspective project the four corners of this patch
            let z_back = z + P3_PATCH_DEPTH * 0.98;
            let hw     = P3_PATCH_HALF_W;
            let hy     = 0.45_f32;

            let (x0s, y0s) = project_3d(-hw, world_y - hy, z);
            let (x1s, y1s) = project_3d( hw, world_y - hy, z);
            let (x2s, y2s) = project_3d( hw, world_y + hy, z);
            let (x3s, y3s) = project_3d(-hw, world_y + hy, z);

            let (x0b, y0b) = project_3d(-hw, world_y - hy, z_back);
            let (x1b, y1b) = project_3d( hw, world_y - hy, z_back);
            let (x2b, y2b) = project_3d( hw, world_y + hy, z_back);
            let (x3b, y3b) = project_3d(-hw, world_y + hy, z_back);

            // Depth-fade: distant patches fade toward background color
            let t_fade = (z / P3_FAR_Z).min(1.0);
            let base_color = if highlight == Some(i) {
                blend(patch.color, 0xFFFFFFFF, 0.4)
            } else {
                patch.color
            };
            let color = blend(base_color, BG_COLOR, t_fade * 0.8);

            // Fill the trapezoid face (front face of the patch box)
            self.fill_quad(
                (x0s, y0s), (x1s, y1s), (x2s, y2s), (x3s, y3s),
                color,
            );

            // Top edge (darker)
            let top_color = blend(color, 0xFF000000, 0.3);
            self.fill_quad(
                (x0s, y0s), (x1s, y1s), (x1b, y1b), (x0b, y0b),
                top_color,
            );

            // Border lines
            let border = blend(0xFF000000, color, 0.4);
            self.draw_line(x0s, y0s, x1s, y1s, border);
            self.draw_line(x0s, y3s, x1s, y2s, border);
            self.draw_line(x0s, y0s, x0b, y0b, border);
            self.draw_line(x1s, y1s, x1b, y1b, border);

            // Digit label at centre of front face
            let cx = ((x0s + x1s) / 2.0) as usize;
            let cy = ((y0s + y3s) / 2.0) as usize;
            if cx + 4 < WIN_W && cy + 4 < WIN_H {
                self.draw_label(&format!("{}", patch.digit), cx.saturating_sub(2), cy, 0xFF000000);
            }
        }
    }

    fn draw_vp_grid(&mut self) {
        // Subtle converging grid lines toward vanishing point
        for i in 0..8 {
            let t   = i as f32 / 7.0;
            let x0  = (WIN_W as f32 * t) as usize;
            let col = blend(0xFF0D0D20, 0xFF1F1F40, t);
            self.draw_line(x0 as f32, WIN_H as f32 - 36.0, P3_VPX, P3_VPY, col);
        }
    }

    fn draw_3d_stitch(&mut self, progress: f32) {
        let patches = (P3_FAR_Z / P3_PATCH_DEPTH) as usize;
        let visible  = (patches as f32 * progress) as usize;
        for i in 0..visible {
            let z    = P3_NEAR_Z + i as f32 * P3_PATCH_DEPTH;
            let (lx, ly) = project_3d(0.0, P3_LEFT_WORLD_Y, z);
            let (rx, ry) = project_3d(0.0, P3_RIGHT_WORLD_Y, z);
            let t_fade   = (z / P3_FAR_Z).min(1.0);
            let c = blend(STITCH_COLOR, BG_COLOR, t_fade * 0.85);
            self.draw_line(lx, ly, rx, ry, c);
            if i % 4 == 0 {
                let mx = ((lx + rx) / 2.0) as usize;
                let my = ((ly + ry) / 2.0) as usize;
                self.draw_diamond(mx, my, 3, c);
            }
        }
    }

    fn draw_3d_scissor(&mut self, sc: &ScissorAnimation) {
        let end = sc.start_patch + (sc.count as f32 * sc.progress) as usize;
        for i in sc.start_patch..end {
            let z = P3_NEAR_Z + i as f32 * P3_PATCH_DEPTH;
            let hw = P3_PATCH_HALF_W;
            let hy = 0.45;
            let (x0, y0) = project_3d(-hw, P3_LEFT_WORLD_Y - hy, z);
            let (x1, y1) = project_3d( hw, P3_LEFT_WORLD_Y + hy, z);
            let (x2, y2) = project_3d(-hw, P3_RIGHT_WORLD_Y - hy, z);
            let (x3, y3) = project_3d( hw, P3_RIGHT_WORLD_Y + hy, z);
            self.draw_line(x0, y0, x1, y1, HIGHLIGHT_COLOR);
            self.draw_line(x2, y2, x3, y3, HIGHLIGHT_COLOR);
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Hand ghost renderer  (3D mode only)
    // ════════════════════════════════════════════════════════════════════════
    //
    // Each hand is defined as a set of line segments in a local 2D coordinate
    // system (palm centre = origin).  We place the left hand in the lower-left
    // quadrant and the right hand in the lower-right quadrant, then apply a
    // pose transform based on the current HandGesture.
    //
    // Finger segments: (tip_x, tip_y) relative to palm, one pair per knuckle.

    fn draw_hand_ghosts(&mut self) {
        let frame = self.frame;
        let gesture = self.hand_gesture;

        // Left hand: lower-left area
        let left_cx  = (WIN_W as f32 * 0.20) as isize;
        let left_cy  = (WIN_H as f32 * 0.72) as isize;
        // Right hand: lower-right area
        let right_cx = (WIN_W as f32 * 0.65) as isize;
        let right_cy = (WIN_H as f32 * 0.72) as isize;

        let (left_pose, right_pose) = gesture_poses(gesture, frame);

        let lc = 0xFFAADDFF;  // blue tint for left hand
        let rc = 0xFFFFBBAA;  // warm tint for right hand

        self.draw_hand(&left_pose,  left_cx,  left_cy,  lc, false);
        self.draw_hand(&right_pose, right_cx, right_cy, rc, true);

        // Label
        let gname = gesture_name(gesture);
        self.draw_label(gname, left_cx as usize, (left_cy + 60) as usize, 0xFFCCCCCC);
    }

    /// Draw a wireframe hand.
    /// `segments` is a list of (x1,y1,x2,y2) in local coords (scale ≈ 40px=1unit).
    fn draw_hand(&mut self, pose: &HandPose, cx: isize, cy: isize, color: u32, mirror: bool) {
        let scale = 38.0_f32;
        let mir   = if mirror { -1.0_f32 } else { 1.0_f32 };

        // Palm outline
        for &(ax, ay, bx, by) in &pose.segments {
            let sx1 = cx + (ax * scale * mir) as isize;
            let sy1 = cy + (ay * scale) as isize;
            let sx2 = cx + (bx * scale * mir) as isize;
            let sy2 = cy + (by * scale) as isize;
            if sx1 >= 0 && sy1 >= 0 && sx2 >= 0 && sy2 >= 0
               && (sx1 as usize) < WIN_W && (sy1 as usize) < WIN_H
               && (sx2 as usize) < WIN_W && (sy2 as usize) < WIN_H {
                self.draw_line(sx1 as f32, sy1 as f32, sx2 as f32, sy2 as f32, color);
            }
        }

        // Palm circle
        self.draw_circle(cx as usize, cy as usize, 10, color);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Snippet tray (shared across all modes)
    // ════════════════════════════════════════════════════════════════════════

    fn draw_tray(&mut self, tray: &SnippetTray, x_origin: usize) {
        self.draw_label("SNIPPETS", x_origin + 8, 10, STITCH_COLOR);
        let mut ey = 32usize;
        for entry in &tray.entries {
            let slide  = entry.slide_in;
            let ex     = x_origin + (TRAY_W as f32 * (1.0 - slide)) as usize;
            if ex < WIN_W {
                self.fill_rect(ex, ey, WIN_W - ex, 50, TEXT_BG);
                self.draw_label(&entry.name, ex + 4, ey + 4, STITCH_COLOR);
                let max_p = 8;
                let pw    = (TRAY_W - 16) / max_p;
                for (j, (lp, rp)) in entry.patches.iter().take(max_p).enumerate() {
                    let px = ex + 4 + j * pw;
                    self.fill_rect(px, ey + 16, pw.saturating_sub(2), 13, lp.color);
                    self.fill_rect(px, ey + 31, pw.saturating_sub(2), 13, rp.color);
                }
            }
            ey += 56;
            if ey + 56 > WIN_H - 36 { break; }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Primitive drawing
    // ════════════════════════════════════════════════════════════════════════

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for row in y..(y+h).min(WIN_H) {
            for col in x..(x+w).min(WIN_W) {
                self.buf[row * WIN_W + col] = color;
            }
        }
    }

    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for col in x..(x+w).min(WIN_W) {
            if y < WIN_H         { self.buf[y           * WIN_W + col] = color; }
            if y+h > 0 && y+h-1 < WIN_H { self.buf[(y+h-1) * WIN_W + col] = color; }
        }
        for row in y..(y+h).min(WIN_H) {
            if x < WIN_W         { self.buf[row * WIN_W + x    ] = color; }
            if x+w > 0 && x+w-1 < WIN_W { self.buf[row * WIN_W + x+w-1] = color; }
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < WIN_W && y < WIN_H {
            self.buf[y * WIN_W + x] = color;
        }
    }

    /// Bresenham line rasteriser.
    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
        let mut x0 = x0 as isize; let mut y0 = y0 as isize;
        let     x1 = x1 as isize; let     y1 = y1 as isize;
        let dx =  (x1-x0).abs(); let dy = -(y1-y0).abs();
        let sx = if x0 < x1 { 1isize } else { -1 };
        let sy = if y0 < y1 { 1isize } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && y0 >= 0 && (x0 as usize) < WIN_W && (y0 as usize) < WIN_H {
                self.buf[y0 as usize * WIN_W + x0 as usize] = color;
            }
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }

    fn draw_circle(&mut self, cx: usize, cy: usize, r: usize, color: u32) {
        let mut x = r as isize; let mut y = 0isize; let mut err = 0isize;
        while x >= y {
            for &(px, py) in &[
                (cx as isize+x, cy as isize+y), (cx as isize-x, cy as isize+y),
                (cx as isize+x, cy as isize-y), (cx as isize-x, cy as isize-y),
                (cx as isize+y, cy as isize+x), (cx as isize-y, cy as isize+x),
                (cx as isize+y, cy as isize-x), (cx as isize-y, cy as isize-x),
            ] {
                if px >= 0 && py >= 0 { self.set_pixel(px as usize, py as usize, color); }
            }
            y += 1; err += 1 + 2*y;
            if 2*(err-x) + 1 > 0 { x -= 1; err += 1 - 2*x; }
        }
    }

    fn draw_diamond(&mut self, cx: usize, cy: usize, r: usize, color: u32) {
        for dy in 0..=r as isize {
            let dx = r as isize - dy;
            for &(sx, sy) in &[
                (cx as isize+dx, cy as isize+dy), (cx as isize-dx, cy as isize+dy),
                (cx as isize+dx, cy as isize-dy), (cx as isize-dx, cy as isize-dy),
            ] {
                if sx >= 0 && sy >= 0 { self.set_pixel(sx as usize, sy as usize, color); }
            }
        }
    }

    /// Fill a convex quadrilateral by scan-line.
    fn fill_quad(
        &mut self,
        p0: (f32,f32), p1: (f32,f32), p2: (f32,f32), p3: (f32,f32),
        color: u32,
    ) {
        // Decompose into two triangles: (p0,p1,p2) and (p0,p2,p3)
        self.fill_triangle(p0, p1, p2, color);
        self.fill_triangle(p0, p2, p3, color);
    }

    /// Barycentric triangle fill.
    fn fill_triangle(&mut self, p0: (f32,f32), p1: (f32,f32), p2: (f32,f32), color: u32) {
        let min_x = p0.0.min(p1.0).min(p2.0).max(0.0) as usize;
        let max_x = p0.0.max(p1.0).max(p2.0).min((WIN_W-1) as f32) as usize;
        let min_y = p0.1.min(p1.1).min(p2.1).max(0.0) as usize;
        let max_y = p0.1.max(p1.1).max(p2.1).min((WIN_H-1) as f32) as usize;

        let denom = (p1.1 - p2.1)*(p0.0 - p2.0) + (p2.0 - p1.0)*(p0.1 - p2.1);
        if denom.abs() < 1e-6 { return; }

        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let w0 = ((p1.1-p2.1)*(fx-p2.0) + (p2.0-p1.0)*(fy-p2.1)) / denom;
                let w1 = ((p2.1-p0.1)*(fx-p2.0) + (p0.0-p2.0)*(fy-p2.1)) / denom;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    self.buf[py * WIN_W + px] = color;
                }
            }
        }
    }

    fn draw_vertical_gradient(&mut self, y0: usize, y1: usize, top: u32, bot: u32) {
        for y in y0..y1.min(WIN_H) {
            let t = (y - y0) as f32 / (y1 - y0) as f32;
            let c = blend(top, bot, t);
            for x in 0..WIN_W { self.buf[y * WIN_W + x] = c; }
        }
    }

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
            cx += 4;
            if cx + 4 > WIN_W { break; }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Hand pose definitions
// ════════════════════════════════════════════════════════════════════════════

/// A hand pose as a list of line segments in local coordinates.
/// X axis = horizontal (+ = away from body midline), Y axis = vertical (+ = down).
struct HandPose {
    segments: Vec<(f32, f32, f32, f32)>,   // (x1, y1, x2, y2)
}

/// Return (left_pose, right_pose) for the given gesture.
fn gesture_poses(gesture: HandGesture, frame: u64) -> (HandPose, HandPose) {
    match gesture {
        HandGesture::Idle     => (idle_hand(), idle_hand()),
        HandGesture::PullLeft => (pull_hand(frame), idle_hand()),
        HandGesture::PullRight=> (idle_hand(), pull_hand(frame)),
        HandGesture::Twist    => (raised_hand(frame, -0.4), raised_hand(frame, 0.4)),
        HandGesture::Clap     => (clap_hand(frame, true), clap_hand(frame, false)),
        HandGesture::Scissors => (scissors_hand(), scissors_hand()),
    }
}

fn gesture_name(g: HandGesture) -> &'static str {
    match g {
        HandGesture::Idle      => "idle",
        HandGesture::PullLeft  => "pull left",
        HandGesture::PullRight => "pull right",
        HandGesture::Twist     => "twist",
        HandGesture::Clap      => "clap",
        HandGesture::Scissors  => "scissors",
    }
}

/// Open palm facing forward — the idle/resting pose.
fn idle_hand() -> HandPose {
    HandPose { segments: vec![
        // Palm outline (rough trapezoid)
        (-0.4, -0.1,  0.4, -0.1),
        ( 0.4, -0.1,  0.35, 0.4),
        ( 0.35, 0.4, -0.35, 0.4),
        (-0.35, 0.4, -0.4, -0.1),
        // Thumb
        (-0.4, -0.1, -0.7, -0.4),
        (-0.7, -0.4, -0.8, -0.65),
        // Index
        (-0.2, -0.1, -0.22, -0.5),
        (-0.22,-0.5, -0.2, -0.85),
        // Middle (longest)
        ( 0.0, -0.1,  0.0, -0.55),
        ( 0.0, -0.55, 0.02,-0.95),
        // Ring
        ( 0.2, -0.1,  0.22,-0.5),
        ( 0.22,-0.5,  0.2, -0.82),
        // Pinky
        ( 0.37,-0.05, 0.38,-0.38),
        ( 0.38,-0.38, 0.36,-0.62),
    ]}
}

/// Fist pulling toward body — fingers curled, wrist angled.
fn pull_hand(frame: u64) -> HandPose {
    let bob = (frame as f32 * 0.12).sin() * 0.05;
    HandPose { segments: vec![
        // Palm
        (-0.35, -0.1 + bob,  0.35, -0.1 + bob),
        ( 0.35, -0.1 + bob,  0.32,  0.4 + bob),
        ( 0.32,  0.4 + bob, -0.32,  0.4 + bob),
        (-0.32,  0.4 + bob, -0.35, -0.1 + bob),
        // Curled fingers (all bent toward palm)
        (-0.7, -0.3 + bob, -0.45, 0.15 + bob),
        (-0.2, -0.42 + bob, -0.25, 0.12 + bob),
        ( 0.0, -0.47 + bob,  0.0,  0.12 + bob),
        ( 0.2, -0.42 + bob,  0.2,  0.12 + bob),
        ( 0.35,-0.32 + bob,  0.34, 0.1  + bob),
    ]}
}

/// Hand raised / crossing — for twist gesture, offset by `x_lean`.
fn raised_hand(frame: u64, x_lean: f32) -> HandPose {
    let wave = (frame as f32 * 0.09).sin() * 0.06;
    let xl   = x_lean;
    HandPose { segments: vec![
        (-0.35 + xl, 0.1 + wave,  0.35 + xl, 0.1 + wave),
        ( 0.35 + xl, 0.1 + wave,  0.32 + xl, 0.5 + wave),
        ( 0.32 + xl, 0.5 + wave, -0.32 + xl, 0.5 + wave),
        (-0.32 + xl, 0.5 + wave, -0.35 + xl, 0.1 + wave),
        // All fingers extended upward
        (-0.65 + xl,-0.3 + wave, -0.5  + xl, 0.08+ wave),
        (-0.2  + xl,-0.5 + wave, -0.2  + xl, 0.08+ wave),
        ( 0.0  + xl,-0.55+ wave,  0.0  + xl, 0.08+ wave),
        ( 0.2  + xl,-0.5 + wave,  0.2  + xl, 0.08+ wave),
        ( 0.35 + xl,-0.35+ wave,  0.35 + xl, 0.08+ wave),
    ]}
}

/// Hands coming together — animate based on frame parity.
fn clap_hand(frame: u64, is_left: bool) -> HandPose {
    let sep  = (frame as f32 * 0.14).sin().abs() * 0.3 + 0.05;
    let xoff = if is_left { -sep } else { sep };
    HandPose { segments: vec![
        // Flat palm moving toward centre
        (-0.35 + xoff,-0.1, 0.35 + xoff,-0.1),
        ( 0.35 + xoff,-0.1, 0.32 + xoff, 0.4),
        ( 0.32 + xoff, 0.4,-0.32 + xoff, 0.4),
        (-0.32 + xoff, 0.4,-0.35 + xoff,-0.1),
        // Fingers extended flat (slightly spread)
        (-0.6  + xoff,-0.3, -0.4  + xoff,-0.12),
        (-0.18 + xoff,-0.5, -0.18 + xoff,-0.12),
        ( 0.02 + xoff,-0.55, 0.02 + xoff,-0.12),
        ( 0.22 + xoff,-0.5,  0.22 + xoff,-0.12),
        ( 0.38 + xoff,-0.32, 0.36 + xoff,-0.10),
    ]}
}

/// Index + middle extended, ring + pinky curled.
fn scissors_hand() -> HandPose {
    HandPose { segments: vec![
        // Palm
        (-0.35,-0.1,  0.35,-0.1),
        ( 0.35,-0.1,  0.32, 0.4),
        ( 0.32, 0.4, -0.32, 0.4),
        (-0.32, 0.4, -0.35,-0.1),
        // Thumb tucked
        (-0.4,-0.1, -0.65,-0.3),
        (-0.65,-0.3,-0.7,-0.5),
        // Index extended and spread outward
        (-0.2,-0.1, -0.3, -0.5),
        (-0.3,-0.5, -0.28,-0.9),
        // Middle extended and spread inward
        ( 0.0,-0.1,  0.1, -0.5),
        ( 0.1,-0.5,  0.08,-0.88),
        // Ring curled
        ( 0.2,-0.1,  0.25, 0.1),
        ( 0.25, 0.1, 0.22, 0.35),
        // Pinky curled
        ( 0.35,-0.05, 0.38, 0.1),
        ( 0.38, 0.1,  0.36, 0.3),
    ]}
}

// ════════════════════════════════════════════════════════════════════════════
// Perspective projection
// ════════════════════════════════════════════════════════════════════════════

/// Project a 3D world point to screen pixel coordinates.
/// World: X = screen-right, Y = screen-up (inverted for screen), Z = depth (into screen).
fn project_3d(wx: f32, wy: f32, wz: f32) -> (f32, f32) {
    let z = wz.max(0.001);
    let sx = P3_VPX + wx * P3_FOCAL / z;
    let sy = P3_VPY - wy * P3_FOCAL / z;   // Y flipped: world-up = screen-up
    (sx, sy)
}

// ════════════════════════════════════════════════════════════════════════════
// Helper: extract stitch progress from StitchPhase
// ════════════════════════════════════════════════════════════════════════════

fn stitch_progress(stitch: &StitchPhase) -> f32 {
    match stitch {
        StitchPhase::Stitching   { progress } => *progress,
        StitchPhase::Stitched                 => 1.0,
        StitchPhase::Unstitching { progress } => 1.0 - progress,
        _                                     => 0.0,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Pulse animation helper
// ════════════════════════════════════════════════════════════════════════════

fn pulse_alpha(frame: u64, lo: f32, hi: f32) -> f32 {
    let t = (frame as f32 * 0.07).sin() * 0.5 + 0.5;
    lo + (hi - lo) * t
}

// ════════════════════════════════════════════════════════════════════════════
// Alpha blend
// ════════════════════════════════════════════════════════════════════════════

fn blend(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |ca: u32, cb: u32| (ca as f32 * (1.0-t) + cb as f32 * t) as u32;
    let ar=(a>>16)&0xFF; let br=(b>>16)&0xFF;
    let ag=(a>> 8)&0xFF; let bg=(b>> 8)&0xFF;
    let ab= a     &0xFF; let bb= b     &0xFF;
    0xFF000000|(lerp(ar,br)<<16)|(lerp(ag,bg)<<8)|lerp(ab,bb)
}

// ════════════════════════════════════════════════════════════════════════════
// Minimal 3×5 bitmap font
// ════════════════════════════════════════════════════════════════════════════

fn char_glyph(c: char) -> [u8; 5] {
    match c {
        '0'=>[0b111,0b101,0b101,0b101,0b111], '1'=>[0b010,0b110,0b010,0b010,0b111],
        '2'=>[0b111,0b001,0b111,0b100,0b111], '3'=>[0b111,0b001,0b111,0b001,0b111],
        '4'=>[0b101,0b101,0b111,0b001,0b001], '5'=>[0b111,0b100,0b111,0b001,0b111],
        '6'=>[0b111,0b100,0b111,0b101,0b111], '7'=>[0b111,0b001,0b001,0b001,0b001],
        '8'=>[0b111,0b101,0b111,0b101,0b111], '9'=>[0b111,0b101,0b111,0b001,0b111],
        'a'|'A'=>[0b111,0b101,0b111,0b101,0b101], 'b'|'B'=>[0b110,0b101,0b110,0b101,0b110],
        'c'|'C'=>[0b111,0b100,0b100,0b100,0b111], 'd'|'D'=>[0b110,0b101,0b101,0b101,0b110],
        'e'|'E'=>[0b111,0b100,0b111,0b100,0b111], 'f'|'F'=>[0b111,0b100,0b111,0b100,0b100],
        'g'|'G'=>[0b111,0b100,0b101,0b101,0b111], 'h'|'H'=>[0b101,0b101,0b111,0b101,0b101],
        'i'|'I'=>[0b111,0b010,0b010,0b010,0b111], 'j'|'J'=>[0b001,0b001,0b001,0b101,0b111],
        'k'|'K'=>[0b101,0b101,0b110,0b101,0b101], 'l'|'L'=>[0b100,0b100,0b100,0b100,0b111],
        'm'|'M'=>[0b101,0b111,0b101,0b101,0b101], 'n'|'N'=>[0b111,0b101,0b101,0b101,0b101],
        'o'|'O'=>[0b111,0b101,0b101,0b101,0b111], 'p'|'P'=>[0b111,0b101,0b111,0b100,0b100],
        'r'|'R'=>[0b110,0b101,0b110,0b101,0b101], 's'|'S'=>[0b111,0b100,0b111,0b001,0b111],
        't'|'T'=>[0b111,0b010,0b010,0b010,0b010], 'u'|'U'=>[0b101,0b101,0b101,0b101,0b111],
        'v'|'V'=>[0b101,0b101,0b101,0b010,0b010], 'w'|'W'=>[0b101,0b101,0b101,0b111,0b101],
        'x'|'X'=>[0b101,0b101,0b010,0b101,0b101], 'y'|'Y'=>[0b101,0b101,0b111,0b010,0b010],
        'z'|'Z'=>[0b111,0b001,0b010,0b100,0b111],
        '/'=>[0b001,0b001,0b010,0b100,0b100], '-'=>[0b000,0b000,0b111,0b000,0b000],
        '.'=>[0b000,0b000,0b000,0b000,0b010], ','=>[0b000,0b000,0b000,0b010,0b100],
        ':'=>[0b000,0b010,0b000,0b010,0b000], '='=>[0b000,0b111,0b000,0b111,0b000],
        '+'=>[0b000,0b010,0b111,0b010,0b000], ' '=>[0b000,0b000,0b000,0b000,0b000],
        _  =>[0b000,0b000,0b010,0b000,0b000],
    }
}
