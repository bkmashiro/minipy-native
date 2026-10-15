//! Minimal, non-execution reference: same JSON content and output bytes via consecutive String additions.
use std::time::Instant;
fn main() {
    for length in ["short", "long"] {
        let content = if length == "long" { "é".repeat(2048) } else { "héllo".to_owned() };
        let reply = serde_json::to_vec(&serde_json::json!({"content": content})).unwrap();
        let expected = format!("[{content}]\n");
        for n in 0..25 {
            let start = Instant::now();
            let object: serde_json::Value = serde_json::from_slice(&reply).unwrap();
            let text = object["content"].as_str().unwrap();
            let out = String::new() + "[" + text + "]" + "\n";
            let ns = start.elapsed().as_nanos();
            assert_eq!(out, expected);
            println!("{{\"length\":\"{length}\",\"warmup\":{},\"ns\":{ns}}}", n == 0);
        }
    }
}
