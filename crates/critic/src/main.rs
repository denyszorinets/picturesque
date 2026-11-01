//! `picturesque-critic`: writes photo critiques with a vision-language model
//! through llama.cpp, for the app's built-in critic (ADR-0016).
//!
//! It runs as a separate process so that a failure inside llama.cpp (a
//! native assert, running out of memory) ends only the critique, and the
//! model's memory is returned when it exits. The app starts it on the first
//! critique and closes its stdin when idle.
//!
//! Protocol (`picturesque_core::critic_protocol`): one `HelperRequest` per
//! line on stdin, one `HelperReply` per line on stdout. Nothing else is
//! written to stdout; llama.cpp's logs are silenced.

mod engine;

use std::io::{BufRead, Write};

use picturesque_core::critic_protocol::{HelperReply, HelperRequest};

fn main() {
    let mut engine = engine::Engine::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<HelperRequest>(&line) {
            Ok(request) => engine.critique(&request),
            Err(e) => HelperReply::Failed {
                message: format!("invalid request: {e}"),
            },
        };
        let text = serde_json::to_string(&reply).expect("replies serialize");
        if writeln!(stdout, "{text}")
            .and_then(|()| stdout.flush())
            .is_err()
        {
            break;
        }
    }
}
