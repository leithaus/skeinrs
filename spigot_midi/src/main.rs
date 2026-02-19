//! Interactive menu for generating MIDI files from transcendental spigot streams.

use spigot_midi::{
    MidiComposer, PitchMap, DurationMap, GeneralMidi, Scale,
    write_multi_track,
};
use dual_spigot::{DualStream, SpigotConfig};
use spigot_stream::Constant;
use std::io::{self, Write};

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        Transcendental Spigot MIDI Composer               ║");
    println!("║  Left stream → duration  |  Right stream → pitch         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    loop {
        println!("  Main menu:");
        println!("    1. Compose single-track MIDI");
        println!("    2. Compose two-track (duet) MIDI");
        println!("    3. Quick demo (π/e → piano, C major, 64 notes)");
        println!("    q. Quit");
        println!();

        match read_line("Choice: ").trim() {
            "1" => compose_single(),
            "2" => compose_duet(),
            "3" => quick_demo(),
            "q" | "quit" => { println!("\nGoodbye!\n"); break; }
            _   => println!("  ⚠  Enter 1–3 or q.\n"),
        }
        println!();
    }
}

fn compose_single() {
    println!("\n  ── Single-track composer ──");

    let left_cfg  = pick_config("LEFT  (duration)");
    let right_cfg = pick_config("RIGHT (pitch)");
    let ds = DualStream::from_configs(left_cfg, right_cfg);

    let bpm: u32 = read_line("  Tempo BPM (default 120): ")
        .trim().parse().unwrap_or(120);
    let bpm = bpm.max(20).min(300);

    let instrument = pick_instrument();
    let pitch_map  = pick_pitch_map();
    let dur_map    = pick_duration_map();

    let n: usize = read_line("  Number of notes (default 64): ")
        .trim().parse().unwrap_or(64);
    let n = n.max(1).min(10_000);

    let velocity: u8 = read_line("  Velocity 0–127 (default 100): ")
        .trim().parse().unwrap_or(100);

    let filename = read_line("  Output filename (default: output.mid): ")
        .trim().to_string();
    let filename = if filename.is_empty() { "output.mid".to_string() } else { filename };

    let desc = format!("{} / {} – {} notes @ {} BPM",
        left_cfg.constant.name(), right_cfg.constant.name(), n, bpm);

    let result = MidiComposer::new(ds)
        .tempo(bpm)
        .instrument_raw(instrument)
        .pitch_map(pitch_map)
        .duration_map(dur_map)
        .velocity(velocity)
        .description(&desc)
        .compose(n);

    match result {
        Err(e) => println!("  ⚠  Error: {}", e),
        Ok(track) => {
            match track.write_file(&filename) {
                Ok(_)  => println!("\n  ✓  Written {} notes to '{}'\n", n, filename),
                Err(e) => println!("  ⚠  File error: {}", e),
            }
        }
    }
}

fn compose_duet() {
    println!("\n  ── Two-track duet composer ──");
    println!("  Track 1 (melody):");
    let l1 = pick_config("    LEFT  (duration)");
    let r1 = pick_config("    RIGHT (pitch)");
    let ds1 = DualStream::from_configs(l1, r1);
    let inst1  = pick_instrument();
    let pmap1  = pick_pitch_map();
    let dmap1  = pick_duration_map();

    println!("\n  Track 2 (bass / accompaniment):");
    let l2 = pick_config("    LEFT  (duration)");
    let r2 = pick_config("    RIGHT (pitch)");
    let ds2 = DualStream::from_configs(l2, r2);
    let inst2  = pick_instrument();
    let pmap2  = pick_pitch_map();
    let dmap2  = pick_duration_map();

    let bpm: u32 = read_line("  Tempo BPM (default 100): ")
        .trim().parse().unwrap_or(100);
    let n: usize = read_line("  Notes per track (default 32): ")
        .trim().parse().unwrap_or(32);
    let filename = read_line("  Output filename (default: duet.mid): ")
        .trim().to_string();
    let filename = if filename.is_empty() { "duet.mid".to_string() } else { filename };

    let t1 = MidiComposer::new(ds1)
        .tempo(bpm).instrument_raw(inst1).pitch_map(pmap1)
        .duration_map(dmap1).channel(0).description("Track 1")
        .compose(n);
    let t2 = MidiComposer::new(ds2)
        .tempo(bpm).instrument_raw(inst2).pitch_map(pmap2)
        .duration_map(dmap2).channel(1).description("Track 2")
        .compose(n);

    match (t1, t2) {
        (Ok(track1), Ok(track2)) => {
            match write_multi_track(&filename, &[track1, track2]) {
                Ok(_)  => println!("\n  ✓  Written duet to '{}'\n", filename),
                Err(e) => println!("  ⚠  File error: {}", e),
            }
        }
        _ => println!("  ⚠  Composition failed."),
    }
}

fn quick_demo() {
    let filename = "pi_e_demo.mid";
    println!("\n  Generating π (duration) × e (pitch) → C major piano, 64 notes…");
    let ds = DualStream::new(Constant::Pi, Constant::E);
    let track = MidiComposer::new(ds)
        .tempo(120)
        .instrument(GeneralMidi::AcousticGrandPiano)
        .pitch_map(PitchMap::major(60))
        .duration_map(DurationMap::musical(480))
        .velocity(100)
        .description("π duration × e pitch – C major")
        .compose(64)
        .unwrap();
    match track.write_file(filename) {
        Ok(_)  => println!("  ✓  Written to '{}'\n", filename),
        Err(e) => println!("  ⚠  {}", e),
    }
}

// ── pickers ──────────────────────────────────────────────────────────────────

fn pick_config(label: &str) -> SpigotConfig {
    let constant = loop {
        println!("  {} — constant:", label);
        for (i, c) in Constant::all().iter().enumerate() {
            println!("    {}. {}", i+1, c.name());
        }
        match read_line("  Choice (1–6): ").trim() {
            "1" => break Constant::Pi,
            "2" => break Constant::E,
            "3" => break Constant::Ln2,
            "4" => break Constant::Liouville,
            "5" => break Constant::Champernowne,
            "6" => break Constant::ThueMorse,
            _   => println!("  ⚠  Enter 1–6."),
        }
    };
    let base: u8 = loop {
        let b = read_line("  Base (2–36, default 10): ")
            .trim().parse::<u8>().unwrap_or(10);
        if b >= 2 && b <= 36 { break b; }
        println!("  ⚠  Base must be 2–36.");
    };
    SpigotConfig::new(constant, base)
}

fn pick_instrument() -> u8 {
    println!("  Instrument family:");
    println!("    1.  Piano family      (0–7)");
    println!("    2.  Mallet/bells      (8–15)");
    println!("    3.  Strings           (40–47)");
    println!("    4.  Brass             (56–63)");
    println!("    5.  Reed / woodwind   (64–79)");
    println!("    6.  Synth lead        (80–87)");
    println!("    7.  Synth pad         (88–95)");
    println!("    8.  Enter raw number  (0–127)");

    match read_line("  Choice (default 1): ").trim() {
        "1" => pick_from_range("Piano",    0,   7),
        "2" => pick_from_range("Mallets",  8,  15),
        "3" => pick_from_range("Strings", 40,  47),
        "4" => pick_from_range("Brass",   56,  63),
        "5" => pick_from_range("Winds",   64,  79),
        "6" => pick_from_range("Synth L", 80,  87),
        "7" => pick_from_range("Synth P", 88,  95),
        "8" => {
            read_line("  Program 0–127: ").trim().parse::<u8>().unwrap_or(0).min(127)
        }
        _   => 0,
    }
}

fn pick_from_range(label: &str, lo: u8, hi: u8) -> u8 {
    println!("  {} programs {}–{}:", label, lo, hi);
    for i in lo..=hi {
        println!("    {:>3}. {}", i, gm_name(i));
    }
    let p: u8 = read_line(&format!("  Program ({lo}–{hi}, default {lo}): "))
        .trim().parse().unwrap_or(lo);
    p.max(lo).min(hi)
}

fn gm_name(p: u8) -> &'static str {
    match p {
        0  => "Acoustic Grand Piano",  1  => "Bright Acoustic Piano",
        2  => "Electric Grand Piano",  3  => "Honky-Tonk Piano",
        4  => "Electric Piano 1",      5  => "Electric Piano 2",
        6  => "Harpsichord",           7  => "Clavinet",
        8  => "Celesta",               9  => "Glockenspiel",
        10 => "Music Box",             11 => "Vibraphone",
        12 => "Marimba",               13 => "Xylophone",
        14 => "Tubular Bells",         15 => "Dulcimer",
        40 => "Violin",                41 => "Viola",
        42 => "Cello",                 43 => "Contrabass",
        44 => "Tremolo Strings",       45 => "Pizzicato Strings",
        46 => "Orchestral Harp",       47 => "Timpani",
        56 => "Trumpet",               57 => "Trombone",
        58 => "Tuba",                  59 => "Muted Trumpet",
        60 => "French Horn",           61 => "Brass Section",
        62 => "Synth Brass 1",         63 => "Synth Brass 2",
        64 => "Soprano Sax",           65 => "Alto Sax",
        66 => "Tenor Sax",             67 => "Baritone Sax",
        68 => "Oboe",                  69 => "English Horn",
        70 => "Bassoon",               71 => "Clarinet",
        72 => "Piccolo",               73 => "Flute",
        74 => "Recorder",              75 => "Pan Flute",
        76 => "Blown Bottle",          77 => "Shakuhachi",
        78 => "Whistle",               79 => "Ocarina",
        80 => "Lead 1 (Square)",       81 => "Lead 2 (Sawtooth)",
        82 => "Lead 3 (Calliope)",     83 => "Lead 4 (Chiff)",
        84 => "Lead 5 (Charang)",      85 => "Lead 6 (Voice)",
        86 => "Lead 7 (Fifths)",       87 => "Lead 8 (Bass+Lead)",
        88 => "Pad 1 (New Age)",       89 => "Pad 2 (Warm)",
        90 => "Pad 3 (Polysynth)",     91 => "Pad 4 (Choir)",
        92 => "Pad 5 (Bowed)",         93 => "Pad 6 (Metallic)",
        94 => "Pad 7 (Halo)",          95 => "Pad 8 (Sweep)",
        _  => "—",
    }
}

fn pick_pitch_map() -> PitchMap {
    let root: u8 = {
        let r = read_line("  Root note MIDI# (0–127, default 60 = middle C): ")
            .trim().parse::<u8>().unwrap_or(60);
        r.min(127)
    };
    println!("  Scale:");
    println!("    1. Major          5. Dorian");
    println!("    2. Minor          6. Phrygian");
    println!("    3. Pentatonic Maj 7. Whole Tone");
    println!("    4. Pentatonic Min 8. Chromatic");
    match read_line("  Choice (default 1): ").trim() {
        "2" => PitchMap::minor(root),
        "3" => PitchMap::pentatonic_major(root),
        "4" => PitchMap::pentatonic_minor(root),
        "5" => PitchMap::custom(root, Scale::dorian()),
        "6" => PitchMap::custom(root, Scale::phrygian()),
        "7" => PitchMap::custom(root, Scale::whole_tone()),
        "8" => PitchMap::chromatic(root),
        _   => PitchMap::major(root),
    }
}

fn pick_duration_map() -> DurationMap {
    let tpq: u32 = read_line("  Ticks per quarter note (default 480): ")
        .trim().parse().unwrap_or(480);
    let tpq = tpq.max(24).min(9600);
    println!("  Duration mapping:");
    println!("    1. Musical note values (32nd → whole)");
    println!("    2. Linear (digit+1 × unit)");
    println!("    3. Exponential (unit × 2^digit)");
    println!("    4. Fixed (every note same length)");
    match read_line("  Choice (default 1): ").trim() {
        "2" => DurationMap::linear(tpq / 4, 10),
        "3" => DurationMap::exponential(tpq / 8, 10),
        "4" => DurationMap::fixed(tpq, 10),
        _   => DurationMap::musical(tpq),
    }
}

fn read_line(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
    buf
}
