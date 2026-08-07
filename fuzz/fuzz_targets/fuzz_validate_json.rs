#![no_main]

use esh_core::Validator;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(v) = serde_json::from_str::<Value>(s) {
            let validator = Validator::new()
                .require("database_url")
                .non_empty("token")
                .pattern("url", "postgres*");
            let _ = validator.validate_json(&v);
        }
    }
});
