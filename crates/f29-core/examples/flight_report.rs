//! Kennwerte des Flugverhaltens als Tabelle (siehe `tests/support/envelope.rs`):
//! `cargo run --release -p f29-core --example flight_report`
#[path = "../tests/support/envelope.rs"]
#[allow(dead_code)]
mod envelope;

fn main() {
    for m in envelope::report() {
        println!("{:<28} {:>9.1} {:<7} {}", m.id, m.value, m.unit, m.label);
    }
}
