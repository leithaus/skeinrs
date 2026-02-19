//! Real-time MIDI playback thread.
//!
//! Notes are generated on the fly from the DualStream zip and sent to a
//! MIDI output port.  Playback can be started and stopped via channels.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use spigot_midi::{PitchMap, DurationMap};
use dual_spigot::DualStream;

// ════════════════════════════════════════════════════════════════════════════
// PlayerCommand — sent to the playback thread
// ════════════════════════════════════════════════════════════════════════════

pub enum PlayerCommand {
    /// Begin streaming notes.
    Play,
    /// Stop after the current note.
    Stop,
    /// Change instrument (MIDI program 0–127).
    SetInstrument(u8),
    /// Change tempo (BPM).
    SetTempo(u32),
    /// Terminate the thread.
    Quit,
}

// ════════════════════════════════════════════════════════════════════════════
// NoteEvent — sent back to the visualizer for highlighting
// ════════════════════════════════════════════════════════════════════════════

/// Emitted by the player for each note played, so the visualizer can
/// animate the currently-playing patch.
#[derive(Clone, Debug)]
pub struct NoteEvent {
    pub pitch:    u8,
    pub duration: u32,   // ticks
    pub velocity: u8,
    /// Stream positions at the time of play.
    pub left_pos:  usize,
    pub right_pos: usize,
}

// ════════════════════════════════════════════════════════════════════════════
// MidiOutput — abstraction over midir / null (for testing)
// ════════════════════════════════════════════════════════════════════════════

trait MidiOut: Send {
    fn program_change(&mut self, channel: u8, program: u8);
    fn note_on(&mut self,  channel: u8, note: u8, velocity: u8);
    fn note_off(&mut self, channel: u8, note: u8);
}

// ── midir backend ─────────────────────────────────────────────────────────

struct MidirOut {
    conn: midir::MidiOutputConnection,
}

impl MidiOut for MidirOut {
    fn program_change(&mut self, channel: u8, program: u8) {
        let _ = self.conn.send(&[0xC0 | (channel & 0x0F), program]);
    }
    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        let _ = self.conn.send(&[0x90 | (channel & 0x0F), note, velocity]);
    }
    fn note_off(&mut self, channel: u8, note: u8) {
        let _ = self.conn.send(&[0x80 | (channel & 0x0F), note, 0]);
    }
}

// ── null backend (used when no MIDI port is available) ────────────────────

struct NullOut;
impl MidiOut for NullOut {
    fn program_change(&mut self, _ch: u8, _p: u8)   {}
    fn note_on(&mut self, _ch: u8, _n: u8, _v: u8)  {}
    fn note_off(&mut self, _ch: u8, _n: u8)          {}
}

// ════════════════════════════════════════════════════════════════════════════
// open_midi_output — enumerate ports and pick first available
// ════════════════════════════════════════════════════════════════════════════

/// Try to open the first available MIDI output port.
/// Falls back to `NullOut` with a warning if none found.
fn open_midi_output() -> Box<dyn MidiOut> {
    let midi_out = match midir::MidiOutput::new("spigot_midi_player") {
        Ok(m)  => m,
        Err(e) => {
            eprintln!("[player] MIDI init error: {} — using null output", e);
            return Box::new(NullOut);
        }
    };

    let ports = midi_out.ports();
    if ports.is_empty() {
        eprintln!("[player] No MIDI output ports found — using null output.");
        eprintln!("[player] Install a MIDI synthesiser such as:");
        eprintln!("         • macOS: built-in CoreMIDI (always available)");
        eprintln!("         • Linux: `timidity -iA` or `fluidsynth`");
        eprintln!("         • Windows: built-in GS Wavetable Synth");
        return Box::new(NullOut);
    }

    // Prefer a softsynth if visible
    let port_idx = ports.iter().enumerate()
        .find(|(_, p)| {
            midi_out.port_name(p).map(|n| {
                let n = n.to_lowercase();
                n.contains("fluid") || n.contains("timidity") ||
                n.contains("microsoft") || n.contains("gm") ||
                n.contains("synth")
            }).unwrap_or(false)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let port = &ports[port_idx];
    let name = midi_out.port_name(port)
        .unwrap_or_else(|_| "Unknown".to_string());
    eprintln!("[player] Opening MIDI port: {}", name);

    match midi_out.connect(port, "spigot-play") {
        Ok(conn) => Box::new(MidirOut { conn }),
        Err(e) => {
            eprintln!("[player] Failed to connect: {} — using null output", e);
            Box::new(NullOut)
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Player — the playback thread
// ════════════════════════════════════════════════════════════════════════════

/// Handle to the MIDI playback thread.
pub struct Player {
    pub cmd_tx:   Sender<PlayerCommand>,
    pub note_rx:  Receiver<NoteEvent>,
}

impl Player {
    /// Spawn the playback thread.
    ///
    /// `stream` is consumed by the thread; `pitch_map` and `duration_map`
    /// configure how zip pairs are turned into notes.
    pub fn spawn(
        stream:       DualStream,
        pitch_map:    PitchMap,
        duration_map: DurationMap,
        instrument:   u8,
        tempo_bpm:    u32,
        velocity:     u8,
        channel:      u8,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        let (note_tx, note_rx) = mpsc::channel::<NoteEvent>();

        thread::spawn(move || {
            player_thread(
                stream, pitch_map, duration_map,
                instrument, tempo_bpm, velocity, channel,
                cmd_rx, note_tx,
            );
        });

        Player { cmd_tx, note_rx }
    }

    pub fn play(&self)  { let _ = self.cmd_tx.send(PlayerCommand::Play);  }
    pub fn stop(&self)  { let _ = self.cmd_tx.send(PlayerCommand::Stop);  }
    pub fn quit(&self)  { let _ = self.cmd_tx.send(PlayerCommand::Quit);  }

    pub fn set_instrument(&self, prog: u8) {
        let _ = self.cmd_tx.send(PlayerCommand::SetInstrument(prog));
    }
    pub fn set_tempo(&self, bpm: u32) {
        let _ = self.cmd_tx.send(PlayerCommand::SetTempo(bpm));
    }

    /// Drain any pending note events (non-blocking).
    pub fn drain_notes(&self) -> Vec<NoteEvent> {
        let mut out = Vec::new();
        while let Ok(n) = self.note_rx.try_recv() { out.push(n); }
        out
    }
}

// ════════════════════════════════════════════════════════════════════════════
// player_thread — the actual loop
// ════════════════════════════════════════════════════════════════════════════

fn player_thread(
    mut stream:       DualStream,
    pitch_map:        PitchMap,
    duration_map:     DurationMap,
    mut instrument:   u8,
    mut tempo_bpm:    u32,
    velocity:         u8,
    channel:          u8,
    cmd_rx:           Receiver<PlayerCommand>,
    note_tx:          Sender<NoteEvent>,
) {
    let mut midi = open_midi_output();
    let mut playing = false;

    // Ticks-per-quarter (matches spigot_midi default)
    const TPQ: u32 = 480;

    midi.program_change(channel, instrument);

    loop {
        // ── drain commands ────────────────────────────────────────────────
        loop {
            match cmd_rx.try_recv() {
                Ok(PlayerCommand::Play)  => {
                    playing = true;
                    midi.program_change(channel, instrument);
                }
                Ok(PlayerCommand::Stop)  => { playing = false; }
                Ok(PlayerCommand::SetInstrument(p)) => {
                    instrument = p;
                    midi.program_change(channel, instrument);
                }
                Ok(PlayerCommand::SetTempo(b)) => { tempo_bpm = b; }
                Ok(PlayerCommand::Quit)  => return,
                Err(_) => break,
            }
        }

        if !playing {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // ── generate next note ────────────────────────────────────────────
        let (left, right) = match stream.zip_next() {
            Some(p) => p,
            None    => { playing = false; continue; }
        };

        let pitch    = pitch_map.note_for(right);
        let ticks    = duration_map.ticks_for(left);
        let millis   = ticks_to_ms(ticks, TPQ, tempo_bpm);

        // Notify visualizer
        let _ = note_tx.send(NoteEvent {
            pitch, duration: ticks, velocity,
            left_pos:  stream.left_pos(),
            right_pos: stream.right_pos(),
        });

        // Play it
        midi.note_on(channel, pitch, velocity);
        thread::sleep(Duration::from_millis(millis));
        midi.note_off(channel, pitch);

        // Brief gap between notes (5% of duration, min 5ms)
        let gap = (millis / 20).max(5);
        thread::sleep(Duration::from_millis(gap));
    }
}

/// Convert ticks to milliseconds given TPQ and BPM.
fn ticks_to_ms(ticks: u32, tpq: u32, bpm: u32) -> u64 {
    // ms = ticks * (60_000 / bpm) / tpq
    let ms_per_beat = 60_000u64 / bpm.max(1) as u64;
    (ticks as u64 * ms_per_beat / tpq.max(1) as u64).max(50)
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_to_ms_quarter_at_120bpm() {
        // At 120 BPM, 480 ticks (quarter note) = 500 ms
        assert_eq!(ticks_to_ms(480, 480, 120), 500);
    }

    #[test]
    fn ticks_to_ms_eighth_at_120bpm() {
        // 240 ticks at 120 BPM = 250 ms
        assert_eq!(ticks_to_ms(240, 480, 120), 250);
    }

    #[test]
    fn ticks_to_ms_min_floor() {
        // Very short durations floor to 50ms
        assert_eq!(ticks_to_ms(1, 480, 120), 50);
    }
}
