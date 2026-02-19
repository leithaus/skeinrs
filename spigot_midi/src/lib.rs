//! # spigot_midi
//!
//! Generate standard MIDI files (Type 0, single track) from a
//! [`DualStream`] zip, where:
//!
//! * **Left digit** → note **duration**
//! * **Right digit** → note **pitch**
//!
//! No external crates are required — MIDI bytes are written directly.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use spigot_midi::{MidiComposer, PitchMap, DurationMap, GeneralMidi};
//! use dual_spigot::{DualStream, SpigotConfig};
//! use spigot_stream::Constant;
//!
//! let ds = DualStream::from_configs(
//!     SpigotConfig::new(Constant::Pi,  10),   // Left  → durations
//!     SpigotConfig::new(Constant::E,   10),   // Right → pitches
//! );
//!
//! let midi = MidiComposer::new(ds)
//!     .tempo(120)
//!     .instrument(GeneralMidi::AcousticGrandPiano)
//!     .pitch_map(PitchMap::major(60))          // C major, root = middle C
//!     .duration_map(DurationMap::musical(480)) // 480 ticks per quarter note
//!     .compose(64)                             // 64 notes
//!     .unwrap();
//!
//! midi.write_file("pi_e_major.mid").unwrap();
//! ```

use std::io::Write;
use dual_spigot::{DualStream, SpigotConfig};

// ════════════════════════════════════════════════════════════════════════════
// General MIDI instrument numbers (Program 0–127)
// ════════════════════════════════════════════════════════════════════════════

/// General MIDI instrument numbers (0-indexed, as sent in Program Change).
///
/// Use [`GeneralMidi::program`] to get the raw `u8` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum GeneralMidi {
    // Piano
    AcousticGrandPiano   = 0,
    BrightAcousticPiano  = 1,
    ElectricGrandPiano   = 2,
    HonkyTonkPiano       = 3,
    ElectricPiano1       = 4,
    ElectricPiano2       = 5,
    Harpsichord          = 6,
    Clavinet             = 7,
    // Chromatic Percussion
    Celesta              = 8,
    Glockenspiel         = 9,
    MusicBox             = 10,
    Vibraphone           = 11,
    Marimba              = 12,
    Xylophone            = 13,
    TubularBells         = 14,
    Dulcimer             = 15,
    // Organ
    DrawbarOrgan         = 16,
    PercussiveOrgan      = 17,
    RockOrgan            = 18,
    ChurchOrgan          = 19,
    ReedOrgan            = 20,
    Accordion            = 21,
    Harmonica            = 22,
    TangoAccordion       = 23,
    // Guitar
    AcousticGuitarNylon  = 24,
    AcousticGuitarSteel  = 25,
    ElectricGuitarJazz   = 26,
    ElectricGuitarClean  = 27,
    ElectricGuitarMuted  = 28,
    OverdrivenGuitar     = 29,
    DistortionGuitar     = 30,
    GuitarHarmonics      = 31,
    // Bass
    AcousticBass         = 32,
    ElectricBassFinger   = 33,
    ElectricBassPick     = 34,
    FretlessBass         = 35,
    SlapBass1            = 36,
    SlapBass2            = 37,
    SynthBass1           = 38,
    SynthBass2           = 39,
    // Strings
    Violin               = 40,
    Viola                = 41,
    Cello                = 42,
    Contrabass           = 43,
    TremoloStrings       = 44,
    PizzicatoStrings     = 45,
    OrchestralHarp       = 46,
    Timpani              = 47,
    // Ensemble
    StringEnsemble1      = 48,
    StringEnsemble2      = 49,
    SynthStrings1        = 50,
    SynthStrings2        = 51,
    ChoirAahs            = 52,
    VoiceOohs            = 53,
    SynthVoice           = 54,
    OrchestraHit         = 55,
    // Brass
    Trumpet              = 56,
    Trombone             = 57,
    Tuba                 = 58,
    MutedTrumpet         = 59,
    FrenchHorn           = 60,
    BrassSection         = 61,
    SynthBrass1          = 62,
    SynthBrass2          = 63,
    // Reed
    SopranoSax           = 64,
    AltoSax              = 65,
    TenorSax             = 66,
    BaritoneSax          = 67,
    Oboe                 = 68,
    EnglishHorn          = 69,
    Bassoon              = 70,
    Clarinet             = 71,
    // Pipe
    Piccolo              = 72,
    Flute                = 73,
    Recorder             = 74,
    PanFlute             = 75,
    BlownBottle          = 76,
    Shakuhachi           = 77,
    Whistle              = 78,
    Ocarina              = 79,
    // Synth Lead
    Lead1Square          = 80,
    Lead2Sawtooth        = 81,
    Lead3Calliope        = 82,
    Lead4Chiff           = 83,
    Lead5Charang         = 84,
    Lead6Voice           = 85,
    Lead7Fifths          = 86,
    Lead8BassLead        = 87,
    // Synth Pad
    Pad1NewAge           = 88,
    Pad2Warm             = 89,
    Pad3Polysynth        = 90,
    Pad4Choir            = 91,
    Pad5Bowed            = 92,
    Pad6Metallic         = 93,
    Pad7Halo             = 94,
    Pad8Sweep            = 95,
    // Synth Effects
    Fx1Rain              = 96,
    Fx2Soundtrack        = 97,
    Fx3Crystal           = 98,
    Fx4Atmosphere        = 99,
    Fx5Brightness        = 100,
    Fx6Goblins           = 101,
    Fx7Echoes            = 102,
    Fx8Scifi             = 103,
    // Ethnic
    Sitar                = 104,
    Banjo                = 105,
    Shamisen             = 106,
    Koto                 = 107,
    Kalimba              = 108,
    BagPipe              = 109,
    Fiddle               = 110,
    Shanai               = 111,
    // Percussive
    TinkleBell           = 112,
    Agogo                = 113,
    SteelDrums           = 114,
    Woodblock            = 115,
    TaikoDrum            = 116,
    MelodicTom           = 117,
    SynthDrum            = 118,
    ReverseCymbal        = 119,
    // Sound Effects
    GuitarFretNoise      = 120,
    BreathNoise          = 121,
    Seashore             = 122,
    BirdTweet            = 123,
    TelephoneRing        = 124,
    Helicopter           = 125,
    Applause             = 126,
    Gunshot              = 127,
}

impl GeneralMidi {
    /// Raw MIDI program number (0–127).
    pub fn program(self) -> u8 { self as u8 }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            GeneralMidi::AcousticGrandPiano  => "Acoustic Grand Piano",
            GeneralMidi::BrightAcousticPiano => "Bright Acoustic Piano",
            GeneralMidi::ElectricGrandPiano  => "Electric Grand Piano",
            GeneralMidi::HonkyTonkPiano      => "Honky-Tonk Piano",
            GeneralMidi::ElectricPiano1      => "Electric Piano 1",
            GeneralMidi::ElectricPiano2      => "Electric Piano 2",
            GeneralMidi::Harpsichord         => "Harpsichord",
            GeneralMidi::Clavinet            => "Clavinet",
            GeneralMidi::Celesta             => "Celesta",
            GeneralMidi::Glockenspiel        => "Glockenspiel",
            GeneralMidi::MusicBox            => "Music Box",
            GeneralMidi::Vibraphone          => "Vibraphone",
            GeneralMidi::Marimba             => "Marimba",
            GeneralMidi::Xylophone           => "Xylophone",
            GeneralMidi::TubularBells        => "Tubular Bells",
            GeneralMidi::Dulcimer            => "Dulcimer",
            GeneralMidi::Violin              => "Violin",
            GeneralMidi::Viola               => "Viola",
            GeneralMidi::Cello               => "Cello",
            GeneralMidi::Trumpet             => "Trumpet",
            GeneralMidi::Trombone            => "Trombone",
            GeneralMidi::FrenchHorn          => "French Horn",
            GeneralMidi::AltoSax             => "Alto Sax",
            GeneralMidi::TenorSax            => "Tenor Sax",
            GeneralMidi::Flute               => "Flute",
            GeneralMidi::Clarinet            => "Clarinet",
            GeneralMidi::Oboe                => "Oboe",
            GeneralMidi::AcousticGuitarNylon => "Acoustic Guitar (nylon)",
            GeneralMidi::AcousticGuitarSteel => "Acoustic Guitar (steel)",
            GeneralMidi::ElectricGuitarJazz  => "Electric Guitar (jazz)",
            GeneralMidi::OverdrivenGuitar    => "Overdriven Guitar",
            GeneralMidi::DistortionGuitar    => "Distortion Guitar",
            GeneralMidi::AcousticBass        => "Acoustic Bass",
            GeneralMidi::ElectricBassFinger  => "Electric Bass (finger)",
            GeneralMidi::SynthBass1          => "Synth Bass 1",
            GeneralMidi::Pad1NewAge          => "Pad 1 (New Age)",
            GeneralMidi::Pad2Warm            => "Pad 2 (Warm)",
            GeneralMidi::Pad4Choir           => "Pad 4 (Choir)",
            GeneralMidi::Lead1Square         => "Lead 1 (Square)",
            GeneralMidi::Lead2Sawtooth       => "Lead 2 (Sawtooth)",
            GeneralMidi::Kalimba             => "Kalimba",
            GeneralMidi::Sitar               => "Sitar",
            GeneralMidi::SteelDrums          => "Steel Drums",
            _                                => "General MIDI Instrument",
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Scale — pitch sets for the PitchMap
// ════════════════════════════════════════════════════════════════════════════

/// A pitch collection used by [`PitchMap`] to turn digit values into
/// MIDI note numbers.
///
/// Scales are defined as semitone intervals from the root.
#[derive(Clone, Debug)]
pub struct Scale {
    /// Semitone offsets from root, e.g. `[0,2,4,5,7,9,11]` for major.
    pub intervals: Vec<u8>,
    pub name: &'static str,
}

impl Scale {
    /// All 12 chromatic semitones.
    pub fn chromatic() -> Self {
        Scale { intervals: (0..12).collect(), name: "Chromatic" }
    }
    /// Major scale (Ionian): W W H W W W H
    pub fn major() -> Self {
        Scale { intervals: vec![0,2,4,5,7,9,11], name: "Major" }
    }
    /// Natural minor (Aeolian): W H W W H W W
    pub fn minor() -> Self {
        Scale { intervals: vec![0,2,3,5,7,8,10], name: "Minor" }
    }
    /// Pentatonic major: W W 3H W 3H
    pub fn pentatonic_major() -> Self {
        Scale { intervals: vec![0,2,4,7,9], name: "Pentatonic Major" }
    }
    /// Pentatonic minor
    pub fn pentatonic_minor() -> Self {
        Scale { intervals: vec![0,3,5,7,10], name: "Pentatonic Minor" }
    }
    /// Dorian mode
    pub fn dorian() -> Self {
        Scale { intervals: vec![0,2,3,5,7,9,10], name: "Dorian" }
    }
    /// Phrygian mode
    pub fn phrygian() -> Self {
        Scale { intervals: vec![0,1,3,5,7,8,10], name: "Phrygian" }
    }
    /// Lydian mode
    pub fn lydian() -> Self {
        Scale { intervals: vec![0,2,4,6,7,9,11], name: "Lydian" }
    }
    /// Mixolydian mode
    pub fn mixolydian() -> Self {
        Scale { intervals: vec![0,2,4,5,7,9,10], name: "Mixolydian" }
    }
    /// Whole-tone scale
    pub fn whole_tone() -> Self {
        Scale { intervals: vec![0,2,4,6,8,10], name: "Whole Tone" }
    }
    /// Diminished (octatonic) scale
    pub fn diminished() -> Self {
        Scale { intervals: vec![0,2,3,5,6,8,9,11], name: "Diminished" }
    }
    /// Custom scale from a list of semitone offsets.
    pub fn custom(intervals: Vec<u8>) -> Self {
        Scale { intervals, name: "Custom" }
    }
    /// Number of pitches in the scale.
    pub fn len(&self) -> usize { self.intervals.len() }
    pub fn is_empty(&self) -> bool { self.intervals.is_empty() }
}

// ════════════════════════════════════════════════════════════════════════════
// PitchMap — maps Right digit (0..base) → MIDI note number (0–127)
// ════════════════════════════════════════════════════════════════════════════

/// Maps a digit value (0..base) to a MIDI note number (0–127).
///
/// Digits index into a [`Scale`] (wrapping across octaves), starting from
/// a configurable root note.
///
/// # Example
/// ```rust
/// use spigot_midi::PitchMap;
///
/// // C major starting at middle C (MIDI 60), base 10
/// let pm = PitchMap::major(60);
/// assert_eq!(pm.note_for(0), 60);  // C4
/// assert_eq!(pm.note_for(1), 62);  // D4
/// assert_eq!(pm.note_for(7), 72);  // C5 (octave wrap)
/// ```
#[derive(Clone, Debug)]
pub struct PitchMap {
    /// MIDI note number for digit 0.
    pub root: u8,
    /// Scale used for mapping.
    pub scale: Scale,
}

impl PitchMap {
    /// Map onto a chromatic scale from `root`.
    pub fn chromatic(root: u8) -> Self {
        PitchMap { root, scale: Scale::chromatic() }
    }
    /// Map onto a major scale from `root`.
    pub fn major(root: u8) -> Self {
        PitchMap { root, scale: Scale::major() }
    }
    /// Map onto a natural minor scale from `root`.
    pub fn minor(root: u8) -> Self {
        PitchMap { root, scale: Scale::minor() }
    }
    /// Map onto a pentatonic major scale from `root`.
    pub fn pentatonic_major(root: u8) -> Self {
        PitchMap { root, scale: Scale::pentatonic_major() }
    }
    /// Map onto a pentatonic minor scale from `root`.
    pub fn pentatonic_minor(root: u8) -> Self {
        PitchMap { root, scale: Scale::pentatonic_minor() }
    }
    /// Map onto a custom scale from `root`.
    pub fn custom(root: u8, scale: Scale) -> Self {
        PitchMap { root, scale }
    }
    /// Map onto a Dorian mode scale from `root`.
    pub fn dorian(root: u8) -> Self {
        PitchMap { root, scale: Scale::dorian() }
    }
    /// Map onto a Phrygian mode scale from `root`.
    pub fn phrygian(root: u8) -> Self {
        PitchMap { root, scale: Scale::phrygian() }
    }
    /// Map onto a whole-tone scale from `root`.
    pub fn whole_tone(root: u8) -> Self {
        PitchMap { root, scale: Scale::whole_tone() }
    }

    /// Resolve digit `d` to a MIDI note number.
    ///
    /// `d` indexes into the scale, wrapping across octaves.  The result
    /// is clamped to 0–127.
    pub fn note_for(&self, d: u8) -> u8 {
        let n = self.scale.len();
        let octave   = (d as usize) / n;
        let degree   = (d as usize) % n;
        let semitone = self.scale.intervals[degree] as usize;
        let note     = self.root as usize + octave * 12 + semitone;
        note.min(127) as u8
    }
}

// ════════════════════════════════════════════════════════════════════════════
// DurationMap — maps Left digit (0..base) → MIDI ticks
// ════════════════════════════════════════════════════════════════════════════

/// Maps a digit value (0..base) to a note duration in MIDI ticks.
///
/// # Built-in strategies
///
/// * [`DurationMap::musical`] — maps digits to standard note values
///   (32nd, 16th, 8th, quarter, half, whole, …), cycling if base > 8.
/// * [`DurationMap::linear`] — digit `d` → `(d+1) * unit_ticks`.
/// * [`DurationMap::exponential`] — digit `d` → `unit_ticks * 2^d`.
/// * [`DurationMap::fixed`] — every digit maps to the same duration
///   (useful for rhythmically uniform output).
/// * [`DurationMap::custom`] — provide your own lookup table.
#[derive(Clone, Debug)]
pub struct DurationMap {
    /// Ticks per entry (indexed by digit value).
    pub table: Vec<u32>,
    /// Human-readable description.
    pub name: &'static str,
}

impl DurationMap {
    /// Musical note values.
    ///
    /// `ticks_per_quarter` is the MIDI resolution (commonly 480).
    /// The table cycles through: 32nd, 16th, dotted-16th, 8th, dotted-8th,
    /// quarter, dotted-quarter, half, dotted-half, whole.
    pub fn musical(ticks_per_quarter: u32) -> Self {
        let q = ticks_per_quarter;
        let table = vec![
            q / 8,          // 32nd note
            q / 4,          // 16th note
            q * 3 / 8,      // dotted 16th
            q / 2,          // 8th note
            q * 3 / 4,      // dotted 8th
            q,              // quarter note
            q * 3 / 2,      // dotted quarter
            q * 2,          // half note
            q * 3,          // dotted half
            q * 4,          // whole note
        ];
        DurationMap { table, name: "Musical" }
    }

    /// Linear: digit `d` → `(d + 1) * unit_ticks`.
    ///
    /// Digit 0 → shortest, digit (base-1) → longest.
    pub fn linear(unit_ticks: u32, base: u8) -> Self {
        let table = (0..base as u32).map(|d| (d + 1) * unit_ticks).collect();
        DurationMap { table, name: "Linear" }
    }

    /// Exponential: digit `d` → `unit_ticks * 2^d`.
    ///
    /// Gives a wide dynamic range; capped at 16× unit_ticks.
    pub fn exponential(unit_ticks: u32, base: u8) -> Self {
        let table = (0..base as u32)
            .map(|d| unit_ticks * (1u32 << d.min(16)))
            .collect();
        DurationMap { table, name: "Exponential" }
    }

    /// Fixed: every digit maps to `ticks`.
    pub fn fixed(ticks: u32, base: u8) -> Self {
        let table = vec![ticks; base as usize];
        DurationMap { table, name: "Fixed" }
    }

    /// Custom lookup table.  `table[d]` is the duration for digit `d`.
    /// `table.len()` should equal `base`.
    pub fn custom(table: Vec<u32>) -> Self {
        DurationMap { table, name: "Custom" }
    }

    /// Ticks for digit `d`; wraps if `d >= table.len()`.
    pub fn ticks_for(&self, d: u8) -> u32 {
        if self.table.is_empty() { return 120; }
        self.table[(d as usize) % self.table.len()]
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Note — a single MIDI note event
// ════════════════════════════════════════════════════════════════════════════

/// A single resolved note: pitch, duration, and velocity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// MIDI note number (0–127).
    pub pitch:    u8,
    /// Duration in MIDI ticks.
    pub duration: u32,
    /// MIDI velocity (0–127).
    pub velocity: u8,
}

// ════════════════════════════════════════════════════════════════════════════
// MidiTrack — resolved note sequence before serialisation
// ════════════════════════════════════════════════════════════════════════════

/// A resolved sequence of [`Note`]s ready for MIDI serialisation.
///
/// Produced by [`MidiComposer::compose`].
pub struct MidiTrack {
    pub notes:             Vec<Note>,
    pub ticks_per_quarter: u16,
    pub tempo_bpm:         u32,
    pub instrument:        u8,
    pub channel:           u8,
    /// Source description for metadata.
    pub description:       String,
}

impl MidiTrack {
    /// Serialise to a standard MIDI Type-0 file and write to `path`.
    pub fn write_file(&self, path: &str) -> std::io::Result<()> {
        let bytes = self.to_bytes();
        let mut f = std::fs::File::create(path)?;
        f.write_all(&bytes)
    }

    /// Serialise to a `Vec<u8>` containing a valid MIDI Type-0 file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let track = self.build_track_chunk();

        let mut out = Vec::new();
        // ── Header chunk ──────────────────────────────────────────────────
        // MThd  length=6  format=0  ntrks=1  division
        out.extend_from_slice(b"MThd");
        out.extend_from_slice(&6u32.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes()); // format 0
        out.extend_from_slice(&1u16.to_be_bytes()); // 1 track
        out.extend_from_slice(&self.ticks_per_quarter.to_be_bytes());

        // ── Track chunk ───────────────────────────────────────────────────
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(track.len() as u32).to_be_bytes());
        out.extend_from_slice(&track);

        out
    }

    fn build_track_chunk(&self) -> Vec<u8> {
        let mut t: Vec<u8> = Vec::new();
        let ch = self.channel & 0x0F;

        // ── Tempo meta-event (delta=0) ────────────────────────────────────
        let micros = 60_000_000u32 / self.tempo_bpm;
        t.push(0x00); // delta time = 0
        t.push(0xFF); // meta
        t.push(0x51); // tempo
        t.push(0x03); // length 3
        t.push(((micros >> 16) & 0xFF) as u8);
        t.push(((micros >>  8) & 0xFF) as u8);
        t.push(( micros        & 0xFF) as u8);

        // ── Track name meta-event ─────────────────────────────────────────
        let name = self.description.as_bytes();
        t.push(0x00);
        t.push(0xFF);
        t.push(0x03); // track name
        write_vlq(&mut t, name.len() as u32);
        t.extend_from_slice(name);

        // ── Program Change (instrument) ───────────────────────────────────
        t.push(0x00); // delta = 0
        t.push(0xC0 | ch);
        t.push(self.instrument);

        // ── Note events ───────────────────────────────────────────────────
        for note in &self.notes {
            // Note On (delta = 0 between consecutive notes)
            t.push(0x00);
            t.push(0x90 | ch);
            t.push(note.pitch);
            t.push(note.velocity);

            // Note Off after `duration` ticks
            write_vlq(&mut t, note.duration);
            t.push(0x80 | ch);
            t.push(note.pitch);
            t.push(0x00);
        }

        // ── End of Track meta-event ───────────────────────────────────────
        t.push(0x00);
        t.push(0xFF);
        t.push(0x2F);
        t.push(0x00);

        t
    }
}

/// Write a MIDI variable-length quantity (VLQ).
fn write_vlq(buf: &mut Vec<u8>, mut value: u32) {
    let mut bytes = [0u8; 4];
    let mut i = 3;
    bytes[i] = (value & 0x7F) as u8;
    value >>= 7;
    while value > 0 {
        i -= 1;
        bytes[i] = ((value & 0x7F) | 0x80) as u8;
        value >>= 7;
    }
    buf.extend_from_slice(&bytes[i..]);
}

// ════════════════════════════════════════════════════════════════════════════
// MidiComposer — the builder
// ════════════════════════════════════════════════════════════════════════════

/// Builder that consumes a [`DualStream`] zip to produce a [`MidiTrack`].
///
/// Left digit  → duration (via [`DurationMap`])
/// Right digit → pitch    (via [`PitchMap`])
///
/// # Builder pattern
///
/// ```rust,no_run
/// use spigot_midi::{MidiComposer, PitchMap, DurationMap, GeneralMidi};
/// use dual_spigot::{DualStream, SpigotConfig};
/// use spigot_stream::Constant;
///
/// let track = MidiComposer::new(
///         DualStream::new(Constant::Pi, Constant::E))
///     .tempo(90)
///     .instrument(GeneralMidi::Vibraphone)
///     .pitch_map(PitchMap::pentatonic_major(60))
///     .duration_map(DurationMap::musical(480))
///     .velocity(90)
///     .channel(0)
///     .compose(32)
///     .unwrap();
///
/// track.write_file("vibraphone.mid").unwrap();
/// ```
pub struct MidiComposer {
    stream:       DualStream,
    tempo_bpm:    u32,
    instrument:   u8,
    pitch_map:    PitchMap,
    duration_map: DurationMap,
    velocity:     u8,
    channel:      u8,
    tpq:          u16,
    description:  String,
}

impl MidiComposer {
    /// Create a new composer from a `DualStream`.
    ///
    /// Defaults: 120 BPM, Acoustic Grand Piano, C major from middle C,
    /// musical durations at 480 ticks/quarter, velocity 100, channel 0.
    pub fn new(stream: DualStream) -> Self {
        MidiComposer {
            stream,
            tempo_bpm:    120,
            instrument:   GeneralMidi::AcousticGrandPiano.program(),
            pitch_map:    PitchMap::major(60),
            duration_map: DurationMap::musical(480),
            velocity:     100,
            channel:      0,
            tpq:          480,
            description:  "spigot_midi".to_string(),
        }
    }

    // ── setters (builder pattern) ─────────────────────────────────────────

    /// Set the tempo in BPM (beats per minute).
    pub fn tempo(mut self, bpm: u32) -> Self {
        assert!(bpm > 0 && bpm <= 300, "tempo must be 1–300 BPM");
        self.tempo_bpm = bpm;
        self
    }

    /// Set the instrument by [`GeneralMidi`] enum value.
    pub fn instrument(mut self, gm: GeneralMidi) -> Self {
        self.instrument = gm.program();
        self
    }

    /// Set the instrument by raw MIDI program number (0–127).
    pub fn instrument_raw(mut self, program: u8) -> Self {
        self.instrument = program.min(127);
        self
    }

    /// Set the pitch mapping (scale + root note).
    pub fn pitch_map(mut self, pm: PitchMap) -> Self {
        self.pitch_map = pm;
        self
    }

    /// Set the duration mapping.
    pub fn duration_map(mut self, dm: DurationMap) -> Self {
        self.duration_map = dm;
        // Keep tpq consistent if the DurationMap was built with a specific tpq.
        self
    }

    /// Set ticks per quarter note (MIDI resolution). Default 480.
    pub fn ticks_per_quarter(mut self, tpq: u16) -> Self {
        assert!(tpq > 0, "ticks_per_quarter must be > 0");
        self.tpq = tpq;
        self
    }

    /// Set note velocity (0–127). Default 100.
    pub fn velocity(mut self, v: u8) -> Self {
        self.velocity = v.min(127);
        self
    }

    /// Set the MIDI channel (0–15). Default 0.
    pub fn channel(mut self, ch: u8) -> Self {
        self.channel = ch & 0x0F;
        self
    }

    /// Set a descriptive label embedded as the MIDI track name.
    pub fn description(mut self, s: &str) -> Self {
        self.description = s.to_string();
        self
    }

    // ── side-specific cursor operations (delegate to DualStream) ──────────

    /// Advance the Left cursor by `n` digits before composing.
    pub fn drop_left(mut self, n: usize) -> Self {
        self.stream.left().drop(n);
        self
    }

    /// Advance the Right cursor by `n` digits before composing.
    pub fn drop_right(mut self, n: usize) -> Self {
        self.stream.right().drop(n);
        self
    }

    /// Swap Left (duration) and Right (pitch) streams.
    pub fn twist(mut self) -> Self {
        self.stream.twist();
        self
    }

    // ── composition ───────────────────────────────────────────────────────

    /// Consume `n` pairs from the zip stream and resolve them into a
    /// [`MidiTrack`].
    ///
    /// Each pair `(left, right)` produces one [`Note`]:
    /// * `left`  → duration via the [`DurationMap`]
    /// * `right` → pitch    via the [`PitchMap`]
    pub fn compose(mut self, n: usize) -> Result<MidiTrack, String> {
        if n == 0 { return Err("n must be > 0".to_string()); }

        let pairs = self.stream.zip_take(n);
        let notes: Vec<Note> = pairs.into_iter().map(|(left, right)| {
            Note {
                pitch:    self.pitch_map.note_for(right),
                duration: self.duration_map.ticks_for(left),
                velocity: self.velocity,
            }
        }).collect();

        Ok(MidiTrack {
            notes,
            ticks_per_quarter: self.tpq,
            tempo_bpm:         self.tempo_bpm,
            instrument:        self.instrument,
            channel:           self.channel,
            description:       self.description,
        })
    }

    /// Like [`compose`] but apply a filter to the zip stream first:
    /// only pairs where `pred` returns true contribute notes.
    /// Exactly `n` pairs are *consumed* from the stream regardless.
    pub fn compose_filtered<P>(mut self, n: usize, mut pred: P)
        -> Result<MidiTrack, String>
    where P: FnMut(u8, u8) -> bool
    {
        if n == 0 { return Err("n must be > 0".to_string()); }

        let pairs = self.stream.zip_take(n);
        let notes: Vec<Note> = pairs.into_iter()
            .filter(|(l, r)| pred(*l, *r))
            .map(|(left, right)| Note {
                pitch:    self.pitch_map.note_for(right),
                duration: self.duration_map.ticks_for(left),
                velocity: self.velocity,
            })
            .collect();

        if notes.is_empty() {
            return Err("filter rejected all notes".to_string());
        }

        Ok(MidiTrack {
            notes,
            ticks_per_quarter: self.tpq,
            tempo_bpm:         self.tempo_bpm,
            instrument:        self.instrument,
            channel:           self.channel,
            description:       self.description,
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Multi-track helper — compose several MidiTracks into a Type-1 MIDI file
// ════════════════════════════════════════════════════════════════════════════

/// Combine multiple [`MidiTrack`]s into a single Type-1 MIDI file.
///
/// All tracks share the first track's `ticks_per_quarter` and `tempo_bpm`.
/// Each track uses its own instrument and channel.
///
/// # Example
/// ```rust,no_run
/// use spigot_midi::{MidiComposer, PitchMap, DurationMap, GeneralMidi, write_multi_track};
/// use dual_spigot::{DualStream, SpigotConfig};
/// use spigot_stream::Constant;
///
/// let t1 = MidiComposer::new(DualStream::new(Constant::Pi, Constant::E))
///     .instrument(GeneralMidi::Flute)
///     .pitch_map(PitchMap::major(60))
///     .compose(32).unwrap();
///
/// let t2 = MidiComposer::new(DualStream::new(Constant::Ln2, Constant::E))
///     .instrument(GeneralMidi::AcousticBass)
///     .pitch_map(PitchMap::minor(36))
///     .compose(32).unwrap();
///
/// write_multi_track("duet.mid", &[t1, t2]).unwrap();
/// ```
pub fn write_multi_track(path: &str, tracks: &[MidiTrack]) -> std::io::Result<()> {
    if tracks.is_empty() { return Ok(()); }
    let bytes = multi_track_bytes(tracks);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)
}

/// Serialise multiple tracks to MIDI Type-1 format bytes.
pub fn multi_track_bytes(tracks: &[MidiTrack]) -> Vec<u8> {
    if tracks.is_empty() { return Vec::new(); }

    let tpq = tracks[0].ticks_per_quarter;
    let n   = tracks.len() as u16;

    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // format 1
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&tpq.to_be_bytes());

    for track in tracks {
        let chunk = track.build_track_chunk();
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        out.extend_from_slice(&chunk);
    }
    out
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use dual_spigot::DualStream;
    use spigot_stream::Constant;

    // ── VLQ encoding ─────────────────────────────────────────────────────
    #[test]
    fn vlq_single_byte() {
        let mut b = Vec::new();
        write_vlq(&mut b, 0x40);
        assert_eq!(b, [0x40]);
    }

    #[test]
    fn vlq_two_bytes() {
        let mut b = Vec::new();
        write_vlq(&mut b, 128);
        assert_eq!(b, [0x81, 0x00]);
    }

    #[test]
    fn vlq_three_bytes() {
        let mut b = Vec::new();
        write_vlq(&mut b, 0x3FFF);   // max 2-byte VLQ value under 3
        assert_eq!(b, [0xFF, 0x7F]);
    }

    // ── PitchMap ─────────────────────────────────────────────────────────
    #[test]
    fn pitch_map_major_root() {
        let pm = PitchMap::major(60); // C major
        assert_eq!(pm.note_for(0), 60); // C
        assert_eq!(pm.note_for(1), 62); // D
        assert_eq!(pm.note_for(2), 64); // E
        assert_eq!(pm.note_for(6), 71); // B
    }

    #[test]
    fn pitch_map_major_octave_wrap() {
        let pm = PitchMap::major(60);
        // Digit 7 = 1 full octave wrap: octave=1, degree=0 → 60+12=72
        assert_eq!(pm.note_for(7), 72);
    }

    #[test]
    fn pitch_map_pentatonic() {
        let pm = PitchMap::pentatonic_major(60);
        assert_eq!(pm.note_for(0), 60);
        assert_eq!(pm.note_for(1), 62);
        assert_eq!(pm.note_for(4), 69); // A4
        assert_eq!(pm.note_for(5), 72); // octave wrap → C5
    }

    #[test]
    fn pitch_map_clamp_at_127() {
        let pm = PitchMap::chromatic(120);
        // digit 9 = semitone 9 from 120 → 129, clamps to 127
        assert_eq!(pm.note_for(9), 127);
    }

    // ── DurationMap ───────────────────────────────────────────────────────
    #[test]
    fn duration_map_musical_quarter() {
        let dm = DurationMap::musical(480);
        // digit 5 = quarter note = 480 ticks
        assert_eq!(dm.ticks_for(5), 480);
    }

    #[test]
    fn duration_map_linear() {
        let dm = DurationMap::linear(120, 10);
        assert_eq!(dm.ticks_for(0), 120);
        assert_eq!(dm.ticks_for(9), 1200);
    }

    #[test]
    fn duration_map_fixed() {
        let dm = DurationMap::fixed(240, 10);
        for d in 0..10 { assert_eq!(dm.ticks_for(d), 240); }
    }

    #[test]
    fn duration_map_wraps() {
        let dm = DurationMap::custom(vec![100, 200, 300]);
        assert_eq!(dm.ticks_for(3), 100); // wraps
        assert_eq!(dm.ticks_for(4), 200);
    }

    // ── GeneralMidi ───────────────────────────────────────────────────────
    #[test]
    fn gm_program_numbers() {
        assert_eq!(GeneralMidi::AcousticGrandPiano.program(), 0);
        assert_eq!(GeneralMidi::Vibraphone.program(), 11);
        assert_eq!(GeneralMidi::Flute.program(), 73);
        assert_eq!(GeneralMidi::Gunshot.program(), 127);
    }

    // ── compose produces correct note count ───────────────────────────────
    #[test]
    fn compose_note_count() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let track = MidiComposer::new(ds).compose(16).unwrap();
        assert_eq!(track.notes.len(), 16);
    }

    // ── compose maps digits correctly ─────────────────────────────────────
    #[test]
    fn compose_first_note() {
        // π[0]=3 (duration), e[0]=2 (pitch)
        // DurationMap::musical(480): digit 3 = dotted-8th = 480*3/4 = 360
        // PitchMap::major(60): digit 2 = E4 = 64
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let track = MidiComposer::new(ds)
            .pitch_map(PitchMap::major(60))
            .duration_map(DurationMap::musical(480))
            .compose(1).unwrap();
        assert_eq!(track.notes[0].pitch,    64);  // E4
        assert_eq!(track.notes[0].duration, 360); // dotted 8th
    }

    // ── MIDI file structure ───────────────────────────────────────────────
    #[test]
    fn midi_bytes_header() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let track = MidiComposer::new(ds).compose(4).unwrap();
        let bytes = track.to_bytes();
        // Starts with MThd
        assert_eq!(&bytes[0..4], b"MThd");
        // Format 0
        assert_eq!(bytes[8], 0);
        assert_eq!(bytes[9], 0);
        // 1 track
        assert_eq!(bytes[10], 0);
        assert_eq!(bytes[11], 1);
    }

    #[test]
    fn midi_bytes_has_mtrk() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let bytes = MidiComposer::new(ds).compose(4).unwrap().to_bytes();
        assert_eq!(&bytes[14..18], b"MTrk");
    }

    #[test]
    fn midi_bytes_ends_with_eot() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let bytes = MidiComposer::new(ds).compose(4).unwrap().to_bytes();
        let n = bytes.len();
        // Last 3 bytes: FF 2F 00
        assert_eq!(&bytes[n-3..], &[0xFF, 0x2F, 0x00]);
    }

    // ── velocity and instrument propagate ─────────────────────────────────
    #[test]
    fn velocity_propagates() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let track = MidiComposer::new(ds).velocity(64).compose(4).unwrap();
        for n in &track.notes { assert_eq!(n.velocity, 64); }
    }

    #[test]
    fn instrument_stored() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        let track = MidiComposer::new(ds)
            .instrument(GeneralMidi::Vibraphone).compose(4).unwrap();
        assert_eq!(track.instrument, 11);
    }

    // ── drop_left shifts pitch stream ────────────────────────────────────
    #[test]
    fn drop_right_shifts_pitch() {
        // Default right stream is E: 2,7,1,8,...
        // After drop_right(1) first pitch digit is 7
        let ds1 = DualStream::new(Constant::Pi, Constant::E);
        let t1 = MidiComposer::new(ds1)
            .pitch_map(PitchMap::chromatic(0))
            .compose(1).unwrap();

        let ds2 = DualStream::new(Constant::Pi, Constant::E);
        let t2 = MidiComposer::new(ds2)
            .drop_right(1)
            .pitch_map(PitchMap::chromatic(0))
            .compose(1).unwrap();

        assert_ne!(t1.notes[0].pitch, t2.notes[0].pitch);
    }

    // ── compose_filtered ─────────────────────────────────────────────────
    #[test]
    fn compose_filtered_count() {
        let ds = DualStream::new(Constant::Pi, Constant::E);
        // Keep only pairs where left digit is odd
        let track = MidiComposer::new(ds)
            .compose_filtered(20, |l, _| l % 2 != 0)
            .unwrap();
        // π[0..20] odd digits: 1,1,9,5,3,5,9,7,9,3,3 → at least 1
        assert!(!track.notes.is_empty());
        assert!(track.notes.len() <= 20);
    }

    // ── multi-track ───────────────────────────────────────────────────────
    #[test]
    fn multi_track_format1_header() {
        let t1 = MidiComposer::new(DualStream::new(Constant::Pi, Constant::E))
            .compose(4).unwrap();
        let t2 = MidiComposer::new(DualStream::new(Constant::Ln2, Constant::E))
            .compose(4).unwrap();
        let bytes = multi_track_bytes(&[t1, t2]);
        assert_eq!(&bytes[0..4], b"MThd");
        assert_eq!(bytes[8], 0); assert_eq!(bytes[9], 1); // format 1
        assert_eq!(bytes[10], 0); assert_eq!(bytes[11], 2); // 2 tracks
    }
}
