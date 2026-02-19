//! Demonstrates spigot_midi: multiple compositions, instruments, scales, and bases.

use spigot_midi::{
    MidiComposer, PitchMap, DurationMap, GeneralMidi, Scale,
    write_multi_track,
};
use dual_spigot::{DualStream, SpigotConfig};
use spigot_stream::Constant;

fn main() {
    println!("\n=== Spigot MIDI Demo ===\n");

    // ── 1. π (duration) × e (pitch) → C major piano ───────────────────────
    println!("1. π/e → Acoustic Grand Piano, C major, 64 notes");
    let ds = DualStream::new(Constant::Pi, Constant::E);
    MidiComposer::new(ds)
        .tempo(120)
        .instrument(GeneralMidi::AcousticGrandPiano)
        .pitch_map(PitchMap::major(60))
        .duration_map(DurationMap::musical(480))
        .description("π duration × e pitch – C major piano")
        .compose(64).unwrap()
        .write_file("01_pi_e_piano.mid").unwrap();
    println!("   → 01_pi_e_piano.mid\n");

    // ── 2. Champernowne / Thue-Morse → vibraphone, pentatonic ─────────────
    println!("2. Champernowne/ThueMorse → Vibraphone, pentatonic major, 48 notes");
    let ds = DualStream::new(Constant::Champernowne, Constant::ThueMorse);
    MidiComposer::new(ds)
        .tempo(100)
        .instrument(GeneralMidi::Vibraphone)
        .pitch_map(PitchMap::pentatonic_major(60))
        .duration_map(DurationMap::linear(120, 10))
        .velocity(90)
        .description("Champernowne dur × ThueMorse pitch – pentatonic vibraphone")
        .compose(48).unwrap()
        .write_file("02_champ_morse_vibraphone.mid").unwrap();
    println!("   → 02_champ_morse_vibraphone.mid\n");

    // ── 3. ln2 (duration) × π (pitch) → cello, D minor ───────────────────
    println!("3. ln2/π → Cello, D minor, 32 notes, 80 BPM");
    let ds = DualStream::new(Constant::Ln2, Constant::Pi);
    MidiComposer::new(ds)
        .tempo(80)
        .instrument(GeneralMidi::Cello)
        .pitch_map(PitchMap::minor(62)) // D minor
        .duration_map(DurationMap::musical(480))
        .velocity(85)
        .description("ln2 duration × π pitch – D minor cello")
        .compose(32).unwrap()
        .write_file("03_ln2_pi_cello.mid").unwrap();
    println!("   → 03_ln2_pi_cello.mid\n");

    // ── 4. Base-16 π duration × base-2 e pitch → synth pad ───────────────
    println!("4. π (hex) / e (binary) → Synth Pad, whole-tone scale, 40 notes");
    let ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi,  16),  // 16 duration values
        SpigotConfig::new(Constant::E,    2),  // binary pitch (2 values, wide octave range)
    );
    MidiComposer::new(ds)
        .tempo(70)
        .instrument(GeneralMidi::Pad2Warm)
        .pitch_map(PitchMap::custom(48, Scale::whole_tone()))
        .duration_map(DurationMap::musical(480))
        .velocity(80)
        .description("π-hex dur × e-bin pitch – whole tone synth pad")
        .compose(40).unwrap()
        .write_file("04_pi_hex_e_bin_pad.mid").unwrap();
    println!("   → 04_pi_hex_e_bin_pad.mid\n");

    // ── 5. Liouville → flute, sparse (mostly zeros give long silences) ────
    println!("5. Liouville (dur) × π (pitch) → Flute, Dorian, 32 notes");
    let ds = DualStream::new(Constant::Liouville, Constant::Pi);
    MidiComposer::new(ds)
        .tempo(60)
        .instrument(GeneralMidi::Flute)
        .pitch_map(PitchMap::custom(62, Scale::dorian()))  // D dorian
        .duration_map(DurationMap::exponential(60, 10))    // long silences for 0s
        .velocity(75)
        .description("Liouville dur × π pitch – D Dorian flute")
        .compose(32).unwrap()
        .write_file("05_liouville_flute.mid").unwrap();
    println!("   → 05_liouville_flute.mid\n");

    // ── 6. Twist: swap duration and pitch sources mid-stream ──────────────
    println!("6. Twist demo: first 32 notes π-dur/e-pitch, then swapped");
    // We can't twist mid-composition currently, so we compose two tracks
    // and concatenate. Alternatively use drop to simulate offset.
    let ds_normal = DualStream::new(Constant::Pi, Constant::E);
    let track_a = MidiComposer::new(ds_normal)
        .tempo(110)
        .instrument(GeneralMidi::ElectricPiano1)
        .pitch_map(PitchMap::major(60))
        .duration_map(DurationMap::musical(480))
        .description("Normal: π-dur × e-pitch")
        .compose(32).unwrap();

    // Twist: e becomes duration, π becomes pitch
    let ds_twisted = DualStream::new(Constant::E, Constant::Pi);
    let track_b = MidiComposer::new(ds_twisted)
        .tempo(110)
        .instrument(GeneralMidi::ElectricPiano2)
        .pitch_map(PitchMap::major(60))
        .duration_map(DurationMap::musical(480))
        .description("Twisted: e-dur × π-pitch")
        .compose(32).unwrap();

    write_multi_track("06_twist_duet.mid", &[track_a, track_b]).unwrap();
    println!("   → 06_twist_duet.mid  (two-track: normal + twisted)\n");

    // ── 7. drop_left shifts the duration stream ───────────────────────────
    println!("7. drop_left(10): π starts at digit 10 for durations");
    let ds = DualStream::new(Constant::Pi, Constant::E);
    MidiComposer::new(ds)
        .drop_left(10)   // skip first 10 π digits
        .tempo(130)
        .instrument(GeneralMidi::Marimba)
        .pitch_map(PitchMap::pentatonic_major(60))
        .duration_map(DurationMap::musical(480))
        .description("π[10..] dur × e pitch – marimba")
        .compose(32).unwrap()
        .write_file("07_drop_left_marimba.mid").unwrap();
    println!("   → 07_drop_left_marimba.mid\n");

    // ── 8. compose_filtered: only pairs where pitch digit > 3 ────────────
    println!("8. Filtered: only notes where pitch digit > 3");
    let ds = DualStream::new(Constant::Pi, Constant::E);
    let track = MidiComposer::new(ds)
        .tempo(140)
        .instrument(GeneralMidi::Lead2Sawtooth)
        .pitch_map(PitchMap::phrygian(60))
        .duration_map(DurationMap::musical(480))
        .description("Filtered: right-digit > 3 only")
        .compose_filtered(100, |_, r| r > 3).unwrap();
    println!("   Notes generated: {} (from 100 consumed pairs)", track.notes.len());
    track.write_file("08_filtered_synth.mid").unwrap();
    println!("   → 08_filtered_synth.mid\n");

    println!("All files written.  Open any .mid in a DAW or media player.");
    println!("Recommended: TiMidity++, VLC, GarageBand, or any GM synthesiser.\n");
}
