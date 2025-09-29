#![no_main]

use hocon_rs::{Config, Value};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|s: &str| {
    let _ = Config::from_str::<Value>(&s, None);
});
