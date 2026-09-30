//! Dev check for the premium voice: `cargo run --release --example premium_check -- <data-dir>`
use blurt_lib::voice::speaker::Speaker;
use std::time::Instant;

fn main() {
    let data = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let sp = Speaker::spawn(data);
    sp.set_premium(true);
    let (tx, rx) = std::sync::mpsc::channel();
    let t = Instant::now();
    sp.say("Oh, that's a great question! [chuckle] Press Command Shift 4, then Space, then click the window.", move || {
        let _ = tx.send(());
    });
    rx.recv().unwrap();
    println!("premium voice finished in {:?}", t.elapsed());
}
