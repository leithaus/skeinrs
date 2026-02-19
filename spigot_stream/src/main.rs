//! Interactive menu for exploring the six transcendental spigot streams.
//! Supports base selection (2–36) for every constant.

use spigot_stream::{Constant, digit_char};
use std::io::{self, Write};

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║       Transcendental Number Spigot Explorer          ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();

    loop {
        print_menu();
        let choice = read_line("Select a constant (1–6, or q to quit): ");

        if choice.trim().eq_ignore_ascii_case("q") {
            println!("\nGoodbye!\n");
            break;
        }

        let constant = match choice.trim() {
            "1" => Constant::Pi,
            "2" => Constant::E,
            "3" => Constant::Ln2,
            "4" => Constant::Liouville,
            "5" => Constant::Champernowne,
            "6" => Constant::ThueMorse,
            _   => { println!("  ⚠  Please enter 1–6 or q.\n"); continue; }
        };

        // Base selection
        let base: u8 = loop {
            let b_str = read_line("  Base (2–36, default 10): ");
            let b = b_str.trim().parse::<u8>().unwrap_or(10);
            if b >= 2 && b <= 36 { break b; }
            println!("  ⚠  Base must be 2–36.");
        };

        let n: usize = read_line("  How many digits? (default 50): ")
            .trim().parse().unwrap_or(50);
        let n = n.max(1).min(10_000);

        println!();
        println!("  ┌─ {} (base {}) ─", constant.name(), base);
        if base == 10 {
            println!("  │  Reference  : {}", constant.approx());
        }
        println!("  │");

        let digits = constant.digits_in_base(base, n);

        let base_label = match base {
            2  => "binary",
            8  => "octal",
            10 => "decimal",
            16 => "hexadecimal",
            _  => "digits",
        };
        println!("  │  {} digits:", base_label);

        // Print integer part, radix point, then fractional digits wrapped at 60
        let first = digits[0];
        print!("  │    {}", digit_char(first));
        if n > 1 {
            print!(".");
            for (i, &d) in digits[1..].iter().enumerate() {
                if i > 0 && i % 60 == 0 {
                    print!("\n  │    ");
                }
                print!("{}", digit_char(d));
            }
        }
        println!();
        println!("  └─ ({} digits emitted)", n);

        // Also show raw digit vec for small n
        if n <= 30 {
            println!();
            println!("  Raw digit vec : {:?}", &digits);
        }
        println!();
    }
}

fn print_menu() {
    let constants = Constant::all();
    println!("  ┌──────────────────────────────────────────────────────┐");
    for (i, c) in constants.iter().enumerate() {
        println!("  │  {}. {:45} │", i + 1, c.name());
    }
    println!("  └──────────────────────────────────────────────────────┘");
    println!();
}

fn read_line(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
    buf
}
