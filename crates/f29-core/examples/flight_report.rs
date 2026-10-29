//! Kennwerte des Flugverhaltens als Tabelle (siehe `f29_core::envelope`):
//! `cargo run --release -p f29-core --example flight_report`
fn main() {
    for m in f29_core::envelope::report() {
        println!("{:<28} {:>9.1} {:<7} {}", m.id, m.value, m.unit, m.label);
    }
}
