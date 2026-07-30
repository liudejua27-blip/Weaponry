use std::io::{self, Read};

use forgecad_core::lower_forge_visual_geometry_program_v2;
use serde_json::Value;

fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("E005 source stdin must be readable");
    let source: Value = serde_json::from_str(&raw).expect("E005 source stdin must be JSON");
    let lowering = lower_forge_visual_geometry_program_v2(&source)
        .expect("E005 authored source must lower through Rust-owned VP203 compiler");
    println!("{}", serde_json::to_string(&lowering).unwrap());
}
