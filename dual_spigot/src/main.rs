//! Interactive dual-stream menu with per-side constant and base selection.

use dual_spigot::{DualStream, SpigotConfig};
use spigot_stream::{Constant, digit_char};
use std::io::{self, Write};

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          Dual Transcendental Spigot Explorer             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let left_cfg  = pick_config("LEFT ");
    let right_cfg = pick_config("RIGHT");
    let mut ds = DualStream::from_configs(left_cfg, right_cfg);

    println!("\n  ✓  {}\n", ds.status());

    loop {
        print_ops_menu();
        let choice = read_line("Command: ").trim().to_ascii_lowercase();

        match choice.as_str() {
            "1" => {
                let n: usize = read_line("  Drop N from LEFT: ").trim().parse().unwrap_or(1);
                ds.left().drop(n);
                println!("  Left cursor now at position {}.", ds.left_pos());
            }
            "2" => {
                let n: usize = read_line("  Drop N from RIGHT: ").trim().parse().unwrap_or(1);
                ds.right().drop(n);
                println!("  Right cursor now at position {}.", ds.right_pos());
            }
            "3" => {
                let n: usize = read_line("  Take N from LEFT: ").trim().parse().unwrap_or(8);
                let v = ds.left().take(n);
                let s: String = v.iter().map(|&d| digit_char(d)).collect();
                println!("  Left  (base {}) : {:?}  \"{}\"", ds.left_base(), v, s);
                println!("  Left pos now    : {}", ds.left_pos());
            }
            "4" => {
                let n: usize = read_line("  Take N from RIGHT: ").trim().parse().unwrap_or(8);
                let v = ds.right().take(n);
                let s: String = v.iter().map(|&d| digit_char(d)).collect();
                println!("  Right (base {}) : {:?}  \"{}\"", ds.right_base(), v, s);
                println!("  Right pos now   : {}", ds.right_pos());
            }
            "5" => {
                let n: usize = read_line("  Zip-take N pairs: ").trim().parse().unwrap_or(10);
                let pairs = ds.zip_take(n);
                let lb = ds.left_base();
                let rb = ds.right_base();
                println!("  (left base {}, right base {})", lb, rb);
                for (i, (l, r)) in pairs.iter().enumerate() {
                    println!("    [{:>4}]  ({}, {})", i, digit_char(*l), digit_char(*r));
                }
                println!("  Left pos: {}  Right pos: {}", ds.left_pos(), ds.right_pos());
            }
            "6" => {
                ds.twist();
                println!("  Twisted!  {}", ds.status());
            }
            "7" => {
                let key  = read_line("  Snippet key: ").trim().to_string();
                let from = read_line("  From position (inclusive): ").trim().parse::<usize>().unwrap_or(0);
                let to   = read_line("  To   position (exclusive): ").trim().parse::<usize>().unwrap_or(10);
                if from > to {
                    println!("  ⚠  from must be ≤ to.");
                } else {
                    ds.snip(&key, from, to);
                    println!("  Stored {} pairs as \"{}\".", to - from, key);
                }
            }
            "8" => {
                let keys = ds.snippet_keys();
                if keys.is_empty() {
                    println!("  No snippets stored yet.");
                    continue;
                }
                let key = if keys.len() == 1 {
                    keys[0].to_string()
                } else {
                    println!("  Stored snippets: {:?}", keys);
                    read_line("  Which key? ").trim().to_string()
                };
                match ds.get_snippet(&key) {
                    None => println!("  ⚠  No snippet named \"{}\".", key),
                    Some(s) => {
                        let lb = ds.left_base();
                        let rb = ds.right_base();
                        println!("  \"{}\" ({} pairs, left base {}, right base {}):",
                                 key, s.len(), lb, rb);
                        for (i, (l, r)) in s.iter().enumerate() {
                            println!("    [{:>4}]  ({}, {})", i, digit_char(*l), digit_char(*r));
                        }
                    }
                }
            }
            "9" => {
                println!("  {}", ds.status());
            }
            "q" | "quit" => {
                println!("\nGoodbye!\n");
                break;
            }
            _ => println!("  ⚠  Unknown command."),
        }
        println!();
    }
}

fn print_ops_menu() {
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  1. Drop N from Left          5. Zip-take N pairs       │");
    println!("  │  2. Drop N from Right         6. Twist (swap Left/Right)│");
    println!("  │  3. Take N from Left          7. Snip range → snippet   │");
    println!("  │  4. Take N from Right         8. View a snippet         │");
    println!("  │                               9. Status    q. Quit      │");
    println!("  └─────────────────────────────────────────────────────────┘");
}

fn pick_config(side: &str) -> SpigotConfig {
    let constant = loop {
        println!("  {} stream — choose constant:", side);
        for (i, c) in Constant::all().iter().enumerate() {
            println!("    {}. {}  ({})", i + 1, c.name(), c.approx());
        }
        match read_line("  Choice (1–6): ").trim() {
            "1" => break Constant::Pi,
            "2" => break Constant::E,
            "3" => break Constant::Ln2,
            "4" => break Constant::Liouville,
            "5" => break Constant::Champernowne,
            "6" => break Constant::ThueMorse,
            _   => println!("  ⚠  Please enter 1–6.\n"),
        }
    };
    let base: u8 = loop {
        let b = read_line(&format!("  {} base (2–36, default 10): ", side))
            .trim().parse::<u8>().unwrap_or(10);
        if b >= 2 && b <= 36 { break b; }
        println!("  ⚠  Base must be 2–36.");
    };
    SpigotConfig::new(constant, base)
}

fn read_line(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
    buf
}
