//! Trefferquoten der Boden-Luft-Raketen je Manöver (siehe
//! `f29_core::sam_report`), optional nur für einzelne Raketen:
//! `cargo run --release -p f29-core --example sam_report [SA-6 …]`
use f29_core::sam_report::{Maneuver, SAMS, hit_rate};

fn main() {
    let only: Vec<String> = std::env::args().skip(1).collect();
    print!("{:<8}", "");
    for m in Maneuver::ALL {
        print!("{:>13}", m.label());
    }
    println!();
    for kind in SAMS.into_iter().filter(|k| only.is_empty() || only.iter().any(|o| o == k.label())) {
        print!("{:<8}", kind.label());
        for m in Maneuver::ALL {
            let (hits, runs) = hit_rate(kind, m);
            print!("{:>13}", format!("{hits}/{runs}"));
        }
        println!();
    }
}
