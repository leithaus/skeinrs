//! leap_spigot — interactive entry point.

use leap_spigot::app::{AppConfig, run};
use dual_spigot::SpigotConfig;
use spigot_stream::Constant;
use spigot_midi::{PitchMap, DurationMap};
use std::io::{self, Write};

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Leap Spigot — Transcendental MIDI Ribbon Controller      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    #[cfg(feature = "leap")]
    println!("  Mode: LeapMotion hardware");
    #[cfg(not(feature = "leap"))]
    println!("  Mode: Keyboard simulation  (use --features leap for hardware)");
    println!();

    let cfg = if std::env::args().any(|a| a == "--quick") {
        println!("  Quick-start: π/e, C major, piano, 120 BPM\n");
        AppConfig::default()
    } else {
        configure_interactively()
    };

    println!();
    println!("  Opening visualizer window…");
    println!();

    if let Err(e) = run(cfg) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn configure_interactively() -> AppConfig {
    println!("  Configure LEFT stream (→ note durations):");
    let left_config  = pick_config();
    println!("  Configure RIGHT stream (→ note pitches):");
    let right_config = pick_config();

    let bpm: u32 = {
        let b = read_line("  Tempo BPM (default 120): ")
            .trim().parse().unwrap_or(120);
        b.max(20).min(300)
    };

    let instrument: u8 = pick_instrument();
    let pitch_map       = pick_pitch_map();
    let duration_map    = pick_duration_map();
    let velocity: u8 = read_line("  Velocity 0–127 (default 100): ")
        .trim().parse().unwrap_or(100).min(127);

    AppConfig {
        left_config,
        right_config,
        pitch_map,
        duration_map,
        instrument,
        tempo_bpm: bpm,
        velocity,
        channel: 0,
        ribbon_capacity: 26,
    }
}

fn pick_config() -> SpigotConfig {
    let constant = loop {
        println!("    1.π  2.e  3.ln2  4.Liouville  5.Champernowne  6.ThueMorse");
        match read_line("    Choice (1–6, default 1): ").trim() {
            "2" => break Constant::E,
            "3" => break Constant::Ln2,
            "4" => break Constant::Liouville,
            "5" => break Constant::Champernowne,
            "6" => break Constant::ThueMorse,
            _   => break Constant::Pi,
        }
    };
    let base: u8 = loop {
        let b = read_line("    Base 2–36 (default 10): ")
            .trim().parse::<u8>().unwrap_or(10);
        if b >= 2 && b <= 36 { break b; }
        println!("    ⚠  2–36 only.");
    };
    SpigotConfig::new(constant, base)
}

fn pick_instrument() -> u8 {
    println!("  Instrument (GM program 0–127):");
    println!("    0=Grand Piano  11=Vibraphone  40=Violin  42=Cello");
    println!("    56=Trumpet  73=Flute  80=Lead Square  88=Pad New Age");
    read_line("  Program (default 0): ").trim().parse::<u8>().unwrap_or(0).min(127)
}

fn pick_pitch_map() -> PitchMap {
    let root: u8 = read_line("  Root note MIDI# (default 60 = C4): ")
        .trim().parse::<u8>().unwrap_or(60).min(127);
    println!("  Scale: 1=Major 2=Minor 3=PentaMaj 4=PentaMin 5=Dorian 6=WholeTone 7=Chromatic");
    match read_line("  Choice (default 1): ").trim() {
        "2" => PitchMap::minor(root),
        "3" => PitchMap::pentatonic_major(root),
        "4" => PitchMap::pentatonic_minor(root),
        "5" => PitchMap::dorian(root),
        "6" => PitchMap::whole_tone(root),
        "7" => PitchMap::chromatic(root),
        _   => PitchMap::major(root),
    }
}

fn pick_duration_map() -> DurationMap {
    let tpq: u32 = read_line("  Ticks/quarter (default 480): ")
        .trim().parse().unwrap_or(480).max(24).min(9600);
    println!("  Duration: 1=Musical  2=Linear  3=Exponential  4=Fixed");
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
