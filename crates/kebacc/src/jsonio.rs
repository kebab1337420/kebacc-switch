use serde_json::Value;
use std::path::Path;

pub use kebacc_core::jsonio::{jwt_payload, map_mut, obj, read, str_of};

const DEEP_LIMIT: usize = 8;

pub fn deep_str(value: &Value, key: &str) -> Option<String> {
    deep_str_within(value, key, DEEP_LIMIT)
}

fn deep_str_within(value: &Value, key: &str, left: usize) -> Option<String> {
    if left == 0 {
        return None;
    }
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(|found| found.as_str()) {
                if !found.is_empty() {
                    return Some(found.to_string());
                }
            }
            map.values()
                .find_map(|nested| deep_str_within(nested, key, left - 1))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|nested| deep_str_within(nested, key, left - 1)),
        _ => None,
    }
}

pub fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut members: Vec<&String> = map.keys().collect();
            members.sort();
            let body: Vec<String> = members
                .iter()
                .map(|name| {
                    format!(
                        "{}:{}",
                        Value::String((*name).clone()),
                        canonical(&map[*name])
                    )
                })
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => {
            let body: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", body.join(","))
        }
        other => other.to_string(),
    }
}

pub fn write(path: &Path, value: &Value) -> std::io::Result<()> {
    kebacc_core::jsonio::write(path, value)?;
    after_write(path);
    Ok(())
}

pub fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    kebacc_core::jsonio::write_text(path, text)?;
    after_write(path);
    Ok(())
}

pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    kebacc_core::jsonio::write_private_bytes(path, bytes)?;
    after_write(path);
    Ok(())
}

fn after_write(path: &Path) {
    crate::provider::protect_new_file(path);
    crate::pool::forget_snapshots();
}
