#![no_main]

use hocon_rs::{Config, Value};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|s: &[u8]| {
    let _ = Config::from_slice::<Value>(&s, None);
});
