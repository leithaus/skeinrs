//! Demonstrates all six streams across multiple bases.

use spigot_stream::{
    PiStream, EStream, Ln2Stream,
    LiouvilleStream, ChampernowneStream, ThueMorseStream,
    Constant, digit_char,
};

fn show(label: &str, digits: &[u8]) {
    let s: String = digits.iter().map(|&d| digit_char(d)).collect();
    println!("   {:<30} {:?}  ({})", label, digits, s);
}

fn main() {
    println!("\n=== Multi-base Transcendental Spigot Demo ===\n");

    // ── 1. π in bases 2, 8, 10, 16 ───────────────────────────────────────
    println!("1. π across bases");
    show("base  2 (binary):",  &PiStream::with_base(2).take(12).collect::<Vec<_>>());
    show("base  8 (octal):",   &PiStream::with_base(8).take(10).collect::<Vec<_>>());
    show("base 10 (decimal):", &PiStream::new().take(10).collect::<Vec<_>>());
    show("base 16 (hex):",     &PiStream::with_base(16).take(10).collect::<Vec<_>>());
    println!("   Formatted hex: {}", PiStream::with_base(16).format_in_base(15));
    // π in hex = 3.243f6a8885a308…
    println!();

    // ── 2. e in bases 2, 10, 16 ───────────────────────────────────────────
    println!("2. e across bases");
    show("base  2:", &EStream::with_base(2).take(12).collect::<Vec<_>>());
    show("base 10:", &EStream::new().take(10).collect::<Vec<_>>());
    show("base 16:", &EStream::with_base(16).take(10).collect::<Vec<_>>());
    println!("   Formatted bin: {}", EStream::with_base(2).format_in_base(14));
    println!();

    // ── 3. ln 2 in bases 2, 10, 16 ───────────────────────────────────────
    println!("3. ln(2) across bases");
    show("base  2:", &Ln2Stream::with_base(2).take(12).collect::<Vec<_>>());
    show("base 10:", &Ln2Stream::new().take(10).collect::<Vec<_>>());
    show("base 16:", &Ln2Stream::with_base(16).take(10).collect::<Vec<_>>());
    println!();

    // ── 4. Liouville — digit sequence is base-invariant ───────────────────
    println!("4. Liouville's constant (digit sequence identical across bases)");
    show("base  2:", &LiouvilleStream::with_base(2).take(26).collect::<Vec<_>>());
    show("base 10:", &LiouvilleStream::new().take(26).collect::<Vec<_>>());
    println!("   (1s appear at positions 1!=1, 2!=2, 3!=6, 4!=24, ...)");
    println!();

    // ── 5. Champernowne — genuinely different in each base ────────────────
    println!("5. Champernowne's constant (different constant per base)");
    show("base  2 (C₂):", &ChampernowneStream::with_base(2).take(15).collect::<Vec<_>>());
    show("base  8 (C₈):", &ChampernowneStream::with_base(8).take(12).collect::<Vec<_>>());
    show("base 10 (C₁₀):", &ChampernowneStream::new().take(12).collect::<Vec<_>>());
    show("base 16 (C₁₆):", &ChampernowneStream::with_base(16).take(15).collect::<Vec<_>>());
    println!("   C₁₆ formatted: {}", ChampernowneStream::with_base(16).format_in_base(16));
    println!();

    // ── 6. Thue–Morse — always bits ───────────────────────────────────────
    println!("6. Prouhet–Thue–Morse (always binary bits)");
    show("bits:", &ThueMorseStream::new().take(24).collect::<Vec<_>>());
    println!("   Formatted: {}", ThueMorseStream::new().format_in_base(17));
    println!();

    // ── 7. Constant enum with base ────────────────────────────────────────
    println!("7. Constant enum: digits_in_base / format_in_base");
    for base in [2u8, 8, 10, 16] {
        let s = Constant::Pi.format_in_base(base, 12);
        println!("   π base {:>2}: {}", base, s);
    }
    println!();

    // ── 8. Combinators work at any base ───────────────────────────────────
    println!("8. Combinators at base 16");
    // Drop the integer part of π, take 8 hex fractional digits
    let frac_hex: Vec<u8> = PiStream::with_base(16).drop(1).take(8).collect();
    let s: String = frac_hex.iter().map(|&d| digit_char(d)).collect();
    println!("   π hex fractional digits [1..9]: {} ({:?})", s, frac_hex);
    // Expected: 243f6a88 → [2,4,3,15,6,10,8,8]

    let sum = EStream::with_base(16).take(8).fold_left(0u32, |a,d| a + d as u32);
    println!("   Sum of first 8 hex digits of e: {}", sum);
}
