#![no_main]

use hocon_rs::{Config, Value};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|s: String| {
    let _ = Config::parse_str::<Value>(&s, None);
});
