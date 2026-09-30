//! Dev check for Ask AI + short-term memory with the default engine:
//! `cargo run --release --example ask_check`
use blurt_lib::{ai, apple, settings::AiSettings};

#[tokio::main]
async fn main() {
    println!("Apple Intelligence: {}", if apple::available() { "ready".to_string() } else { apple::status() });
    let s = AiSettings::default();
    let first = ai::ask(&s, "write a one line text to Sara saying the landing page is ready thursday", None, None, None)
        .await
        .unwrap();
    println!("1st: {first}");
    let recent = format!("- They asked: write a one line text to Sara saying the landing page is ready thursday\n  You wrote: {first}");
    let second = ai::ask(&s, "now make it more excited", None, None, Some(&recent)).await.unwrap();
    println!("2nd (with memory): {second}");
}
