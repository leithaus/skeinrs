//! Top-level application state machine.
//!
//! `AppState` owns the `DualStream`, the two `RibbonState`s, the `Player`,
//! and the `SnippetTray`.  It processes `GestureEvent`s and drives the
//! visualizer each frame.

use std::sync::mpsc::{self, TryRecvError};
use std::io::{self, Write};

use dual_spigot::{DualStream, SpigotConfig};
use spigot_stream::Constant;
use spigot_midi::{PitchMap, DurationMap, GeneralMidi};

use crate::gesture::{GestureEvent, SimInput, SimGestureSource, spawn_gesture_source};
use crate::ribbon::{RibbonState, StitchPhase, SnippetTray, ScissorAnimation, Patch};
use crate::player::Player;
use crate::visualizer::{Visualizer, WIN_W};

// ════════════════════════════════════════════════════════════════════════════
// AppConfig
// ════════════════════════════════════════════════════════════════════════════

/// Configuration for the full application.
pub struct AppConfig {
    pub left_config:   SpigotConfig,
    pub right_config:  SpigotConfig,
    pub pitch_map:     PitchMap,
    pub duration_map:  DurationMap,
    pub instrument:    u8,
    pub tempo_bpm:     u32,
    pub velocity:      u8,
    pub channel:       u8,
    /// Number of patches kept in each ribbon's visible buffer.
    pub ribbon_capacity: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            left_config:     SpigotConfig::new(Constant::Pi, 10),
            right_config:    SpigotConfig::new(Constant::E,  10),
            pitch_map:       PitchMap::major(60),
            duration_map:    DurationMap::musical(480),
            instrument:      GeneralMidi::AcousticGrandPiano.program(),
            tempo_bpm:       120,
            velocity:        100,
            channel:         0,
            ribbon_capacity: WIN_W / 48 + 2,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Playback state
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState { Stopped, Playing }

// ════════════════════════════════════════════════════════════════════════════
// AppState
// ════════════════════════════════════════════════════════════════════════════

pub struct AppState {
    // ── stream state ─────────────────────────────────────────────────────
    dual:         DualStream,
    left_ribbon:  RibbonState,
    right_ribbon: RibbonState,

    // ── playback ─────────────────────────────────────────────────────────
    player:       Player,
    play_state:   PlayState,
    stitch:       StitchPhase,

    // ── snippet ───────────────────────────────────────────────────────────
    tray:         SnippetTray,
    scissor_anim: Option<ScissorAnimation>,
    snip_start:   usize,   // left-ribbon patch index where snip begins

    // ── note highlight ────────────────────────────────────────────────────
    note_highlight: Option<usize>,

    // ── status message ────────────────────────────────────────────────────
    pub status:   String,

    // ── snippet name input ────────────────────────────────────────────────
    /// True while waiting for the user to type a snippet name.
    awaiting_snippet_name: bool,
    snippet_name_buf:      String,

    // ── instrument / tempo ───────────────────────────────────────────────
    instrument: u8,
    tempo_bpm:  u32,
}

impl AppState {
    pub fn new(cfg: AppConfig) -> Self {
        let left_label  = format!("{} base {}", cfg.left_config.constant.name(),  cfg.left_config.base);
        let right_label = format!("{} base {}", cfg.right_config.constant.name(), cfg.right_config.base);

        let dual = DualStream::from_configs(cfg.left_config, cfg.right_config);

        // Player gets its own independent DualStream starting at position 0.
        let player_dual = DualStream::from_configs(cfg.left_config, cfg.right_config);
        let player = Player::spawn(
            player_dual,
            cfg.pitch_map.clone(),
            cfg.duration_map.clone(),
            cfg.instrument,
            cfg.tempo_bpm,
            cfg.velocity,
            cfg.channel,
        );

        let mut left_ribbon  = RibbonState::new(cfg.ribbon_capacity, cfg.left_config.base,  &left_label);
        let mut right_ribbon = RibbonState::new(cfg.ribbon_capacity, cfg.right_config.base, &right_label);

        // Pre-fill ribbons with initial digits so they're not empty on launch.
        let mut pre = DualStream::from_configs(cfg.left_config, cfg.right_config);
        for i in 0..cfg.ribbon_capacity {
            if let Some((l, r)) = pre.zip_next() {
                left_ribbon.push(l, i);
                right_ribbon.push(r, i);
            }
        }

        AppState {
            dual,
            left_ribbon,
            right_ribbon,
            player,
            play_state:    PlayState::Stopped,
            stitch:        StitchPhase::Unstitched,
            tray:          SnippetTray::default(),
            scissor_anim:  None,
            snip_start:    0,
            note_highlight: None,
            status:        format!("Ready — Left: {}  Right: {}", left_label, right_label),
            awaiting_snippet_name: false,
            snippet_name_buf:      String::new(),
            instrument: cfg.instrument,
            tempo_bpm:  cfg.tempo_bpm,
        }
    }

    // ── process one GestureEvent ─────────────────────────────────────────

    pub fn handle_gesture(&mut self, event: GestureEvent) {
        match event {
            // ── Pull Left ─────────────────────────────────────────────────
            GestureEvent::PullLeft { steps, velocity } => {
                for _ in 0..steps {
                    if let Some(d) = self.dual.left().next() {
                        let pos = self.dual.left_pos();
                        self.left_ribbon.push(d, pos);
                    }
                }
                self.left_ribbon.kick(velocity);
                self.status = format!(
                    "Pull LEFT ×{}  (vel={:.2})  pos={}",
                    steps, velocity, self.dual.left_pos()
                );
            }

            // ── Pull Right ────────────────────────────────────────────────
            GestureEvent::PullRight { steps, velocity } => {
                for _ in 0..steps {
                    if let Some(d) = self.dual.right().next() {
                        let pos = self.dual.right_pos();
                        self.right_ribbon.push(d, pos);
                    }
                }
                self.right_ribbon.kick(velocity);
                self.status = format!(
                    "Pull RIGHT ×{}  (vel={:.2})  pos={}",
                    steps, velocity, self.dual.right_pos()
                );
            }

            // ── Twist ─────────────────────────────────────────────────────
            GestureEvent::Twist => {
                self.dual.twist();
                std::mem::swap(&mut self.left_ribbon, &mut self.right_ribbon);
                // Update labels
                let ll = format!("{} base {}", self.dual.left_constant().name(),
                                              self.dual.left_base());
                let rl = format!("{} base {}", self.dual.right_constant().name(),
                                              self.dual.right_base());
                self.left_ribbon.label  = ll.clone();
                self.right_ribbon.label = rl.clone();
                self.status = format!("TWIST — Left now: {}  Right now: {}", ll, rl);
            }

            // ── Clap → begin MIDI ─────────────────────────────────────────
            GestureEvent::Clap => {
                if self.play_state == PlayState::Stopped {
                    self.play_state = PlayState::Playing;
                    self.stitch = StitchPhase::Stitching { progress: 0.0 };
                    self.player.play();
                    self.status = "CLAP — MIDI playback started ♪".to_string();
                }
            }

            // ── Unclap → stop MIDI ────────────────────────────────────────
            GestureEvent::Unclap => {
                if self.play_state == PlayState::Playing {
                    self.play_state = PlayState::Stopped;
                    self.stitch = StitchPhase::Unstitching { progress: 0.0 };
                    self.player.stop();
                    self.status = "UN-CLAP — MIDI playback stopped".to_string();
                }
            }

            // ── Scissors → snip ───────────────────────────────────────────
            GestureEvent::Scissors { name } => {
                self.do_snip(&name);
            }

            GestureEvent::Quit => { /* handled in run loop */ }
        }
    }

    /// Perform a snip: snapshot `from..to` absolute positions.
    pub fn do_snip(&mut self, name: &str) {
        // Snip from left_pos to left_pos + visible_patches
        let from = self.dual.left_pos().saturating_sub(self.left_ribbon.patches.len());
        let to   = self.dual.left_pos();
        let count = to - from;

        self.dual.snip(name, from, to);

        // Collect patch pairs for the tray
        let pairs: Vec<(Patch, Patch)> = self.left_ribbon.patches.iter()
            .zip(self.right_ribbon.patches.iter())
            .map(|(l, r)| (l.clone(), r.clone()))
            .collect();

        self.tray.deposit(name, pairs);

        // Trigger scissor animation
        self.scissor_anim = Some(ScissorAnimation::new(0, count.min(self.left_ribbon.capacity)));
        self.status = format!("SNIP \"{}\" — {} pairs [{}, {}) saved to tray", name, count, from, to);
    }

    // ── Per-frame tick ────────────────────────────────────────────────────

    pub fn tick(&mut self) {
        // Animate ribbons
        self.left_ribbon.tick(48.0);
        self.right_ribbon.tick(48.0);

        // Advance stitch animation
        self.stitch.tick();

        // Advance scissor animation
        if let Some(ref mut sc) = self.scissor_anim {
            sc.tick();
            if sc.done() { self.scissor_anim = None; }
        }

        // Tray animations
        self.tray.tick();

        // Drain note events from the player
        let notes = self.player.drain_notes();
        if let Some(last) = notes.last() {
            // Find the ribbon patch closest to the left_pos
            self.note_highlight = self.left_ribbon.patches.iter().position(|p| {
                p.position + 1 >= last.left_pos
            });
            self.status = format!(
                "♪ pitch={} duration={}t  L-pos={}  R-pos={}",
                last.pitch, last.duration, last.left_pos, last.right_pos
            );
        }
    }

    // ── Accessors for the render loop ─────────────────────────────────────

    pub fn left_ribbon(&self)     -> &RibbonState   { &self.left_ribbon }
    pub fn right_ribbon(&self)    -> &RibbonState   { &self.right_ribbon }
    pub fn stitch(&self)          -> &StitchPhase   { &self.stitch }
    pub fn tray(&self)            -> &SnippetTray   { &self.tray }
    pub fn scissor_anim(&self)    -> &Option<ScissorAnimation> { &self.scissor_anim }
    pub fn note_highlight(&self)  -> Option<usize>  { self.note_highlight }
    pub fn is_playing(&self)      -> bool           { self.play_state == PlayState::Playing }
}

// ════════════════════════════════════════════════════════════════════════════
// run() — the main application loop
// ════════════════════════════════════════════════════════════════════════════

/// Run the full application.
///
/// This is the entry point called from `main.rs`.  It creates the visualizer,
/// the gesture source (simulation by default, hardware with `--feature leap`),
/// and drives the event/render loop at ~60 fps.
pub fn run(cfg: AppConfig) -> Result<(), String> {
    // ── Sim gesture channel ───────────────────────────────────────────────
    let (sim_tx, sim_rx) = mpsc::channel::<SimInput>();
    let gesture_rx = spawn_gesture_source(SimGestureSource { rx: sim_rx });

    // ── Visualizer (owns the window and the sim input sender) ────────────
    let mut vis = Visualizer::new(sim_tx)?;

    // ── App state ─────────────────────────────────────────────────────────
    let mut app = AppState::new(cfg);

    // ── Main loop ─────────────────────────────────────────────────────────
    while vis.is_open() {
        // 1. Poll window input → translate to SimInput
        if !vis.poll_input() { break; }

        // When S is pressed, poll_input sends SimInput::KeyDown(Scissors).
        // The SimGestureSource forwards it as GestureEvent::Scissors { name: "" }.
        // We intercept that here to collect the name from stdin without
        // blocking the gesture thread.

        // 3. Drain gesture events
        loop {
            match gesture_rx.try_recv() {
                Ok(GestureEvent::Quit) => return Ok(()),
                Ok(GestureEvent::Scissors { name }) => {
                    // Name already collected (hw mode) or we need to prompt (sim mode)
                    let n = if name.is_empty() {
                        print!("  Snippet name: ");
                        io::stdout().flush().ok();
                        let mut buf = String::new();
                        io::stdin().read_line(&mut buf).ok();
                        buf.trim().to_string()
                    } else {
                        name
                    };
                    app.handle_gesture(GestureEvent::Scissors { name: n });
                }
                Ok(evt) => app.handle_gesture(evt),
                Err(TryRecvError::Empty)        => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // 4. Per-frame logic
        app.tick();

        // 5. Render
        vis.render(
            app.left_ribbon(),
            app.right_ribbon(),
            app.stitch(),
            app.tray(),
            app.scissor_anim(),
            &app.status,
            app.is_playing(),
            app.note_highlight(),
        );
    }

    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> AppState {
        AppState::new(AppConfig::default())
    }

    #[test]
    fn pull_left_advances_ribbon() {
        let mut app = make_app();
        let before = app.dual.left_pos();
        app.handle_gesture(GestureEvent::PullLeft { steps: 3, velocity: 0.5 });
        assert_eq!(app.dual.left_pos(), before + 3);
    }

    #[test]
    fn pull_right_does_not_move_left() {
        let mut app = make_app();
        let lbefore = app.dual.left_pos();
        app.handle_gesture(GestureEvent::PullRight { steps: 5, velocity: 0.5 });
        assert_eq!(app.dual.left_pos(), lbefore);
        assert_eq!(app.dual.right_pos(), 5);
    }

    #[test]
    fn twist_swaps_labels() {
        let mut app = make_app();
        let ll_before = app.left_ribbon.label.clone();
        let rl_before = app.right_ribbon.label.clone();
        app.handle_gesture(GestureEvent::Twist);
        assert_ne!(app.left_ribbon.label, ll_before);
        // After twist, what was right is now left
        assert_eq!(app.left_ribbon.label, rl_before);
    }

    #[test]
    fn clap_starts_playing() {
        let mut app = make_app();
        assert_eq!(app.play_state, PlayState::Stopped);
        app.handle_gesture(GestureEvent::Clap);
        assert_eq!(app.play_state, PlayState::Playing);
    }

    #[test]
    fn unclap_stops_playing() {
        let mut app = make_app();
        app.handle_gesture(GestureEvent::Clap);
        app.handle_gesture(GestureEvent::Unclap);
        assert_eq!(app.play_state, PlayState::Stopped);
    }

    #[test]
    fn clap_unclap_stitch_phases() {
        let mut app = make_app();
        app.handle_gesture(GestureEvent::Clap);
        assert!(matches!(app.stitch, StitchPhase::Stitching { .. }));
        app.handle_gesture(GestureEvent::Unclap);
        assert!(matches!(app.stitch, StitchPhase::Unstitching { .. }));
    }

    #[test]
    fn snip_deposits_to_tray() {
        let mut app = make_app();
        // Advance so there are digits to snip
        app.handle_gesture(GestureEvent::PullLeft  { steps: 10, velocity: 0.5 });
        app.handle_gesture(GestureEvent::PullRight { steps: 10, velocity: 0.5 });
        app.do_snip("test_snip");
        assert_eq!(app.tray.entries.len(), 1);
        assert_eq!(app.tray.entries[0].name, "test_snip");
    }

    #[test]
    fn snip_stored_in_dual() {
        let mut app = make_app();
        app.handle_gesture(GestureEvent::PullLeft  { steps: 5, velocity: 0.5 });
        app.do_snip("my_snip");
        assert!(app.dual.get_snippet("my_snip").is_some());
    }

    #[test]
    fn scissor_animation_triggered_by_snip() {
        let mut app = make_app();
        app.handle_gesture(GestureEvent::PullLeft { steps: 5, velocity: 0.5 });
        app.do_snip("anim_test");
        assert!(app.scissor_anim.is_some());
    }

    #[test]
    fn tick_advances_stitch_animation() {
        let mut app = make_app();
        app.handle_gesture(GestureEvent::Clap);
        assert!(matches!(app.stitch, StitchPhase::Stitching { .. }));
        for _ in 0..100 { app.tick(); }
        assert_eq!(app.stitch, StitchPhase::Stitched);
    }
}
