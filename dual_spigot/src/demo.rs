//! Demonstrates DualStream with mixed-base configurations.

use dual_spigot::{DualStream, SpigotConfig};
use spigot_stream::{Constant, digit_char};

fn show_pairs(label: &str, pairs: &[(u8,u8)]) {
    let s: String = pairs.iter()
        .map(|(l,r)| format!("({},{})", digit_char(*l), digit_char(*r)))
        .collect::<Vec<_>>().join(" ");
    println!("   {}: {}", label, s);
}

fn main() {
    println!("\n=== DualStream Multi-base Demo ===\n");

    // ── 1. Base-10 baseline ───────────────────────────────────────────────
    println!("1. π (base 10) vs e (base 10)");
    let mut ds = DualStream::new(Constant::Pi, Constant::E);
    show_pairs("first 8 pairs", &ds.zip_take(8));
    println!("   {}\n", ds.status());

    // ── 2. π hex vs e binary ─────────────────────────────────────────────
    println!("2. π (base 16) vs e (base 2)");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi, 16),
        SpigotConfig::new(Constant::E,   2),
    );
    show_pairs("first 8 pairs", &ds.zip_take(8));
    // Left digits are hex (0–15), right are bits (0/1)
    println!("   left-base={}, right-base={}\n", ds.left_base(), ds.right_base());

    // ── 3. Independent side ops with different bases ───────────────────────
    println!("3. Drop 5 from Left (hex π), take 4 from Right (binary e)");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi, 16),
        SpigotConfig::new(Constant::E,   2),
    );
    ds.left().drop(5);
    let re = ds.right().take(4);
    println!("   Right binary e digits: {:?}", re);
    println!("   Left pos={}, Right pos={}", ds.left_pos(), ds.right_pos());
    let p = ds.zip_next().unwrap();
    println!("   Next zip: ({}, {}) — left=π_hex[5]={}, right=e_bin[4]={}",
             digit_char(p.0), digit_char(p.1), digit_char(p.0), digit_char(p.1));
    println!();

    // ── 4. Twist swaps both constant and base ─────────────────────────────
    println!("4. Twist swaps constant AND base");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi, 16),
        SpigotConfig::new(Constant::E,   2),
    );
    ds.left().drop(3);
    println!("   Before: {}", ds.status());
    ds.twist();
    println!("   After:  {}", ds.status());
    let p = ds.zip_next().unwrap();
    println!("   Next zip after twist: ({}, {})  — e_bin[0]={}, π_hex[3]={}",
             digit_char(p.0), digit_char(p.1), digit_char(p.0), digit_char(p.1));
    println!();

    // ── 5. Snip in mixed-base context ────────────────────────────────────
    println!("5. Snip with mixed bases");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi, 16),
        SpigotConfig::new(Constant::E,   2),
    );
    ds.left().drop(10); // advance live cursor
    ds.snip("hex_pi_bin_e_0_to_8", 0, 8);  // absolute snapshot
    ds.snip("hex_pi_bin_e_4_to_12", 4, 12);

    println!("   Live cursors unaffected: L={}, R={}", ds.left_pos(), ds.right_pos());
    let s1 = ds.get_snippet("hex_pi_bin_e_0_to_8").unwrap();
    show_pairs("  abs 0..8", s1);
    let s2 = ds.get_snippet("hex_pi_bin_e_4_to_12").unwrap();
    show_pairs(" abs 4..12", s2);
    println!();

    // ── 6. Champernowne base 2 vs Liouville ──────────────────────────────
    println!("6. Champernowne base-2 (Left) vs Liouville (Right, base-invariant)");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Champernowne, 2),
        SpigotConfig::new(Constant::Liouville, 10),
    );
    show_pairs("first 12 pairs", &ds.zip_take(12));
    println!();

    // ── 7. All-base zip fold ──────────────────────────────────────────────
    println!("7. Sum of digit values: π_hex vs e_base8, first 10 pairs");
    let mut ds = DualStream::from_configs(
        SpigotConfig::new(Constant::Pi, 16),
        SpigotConfig::new(Constant::E,   8),
    );
    let (lsum, rsum) = ds.zip_fold_n(10, (0u32,0u32), |(la,ra),(l,r)| {
        (la + l as u32, ra + r as u32)
    });
    println!("   Left (hex) digit sum: {}  Right (oct) digit sum: {}", lsum, rsum);
    println!();
}
