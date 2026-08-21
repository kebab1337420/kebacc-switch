use serde_json::Value;
use std::path::Path;

pub use kebacc_core::jsonio::{jwt_payload, map_mut, obj, read, str_of};

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

fn after_write(path: &Path) {
    crate::provider::protect_new_file(path);
    crate::pool::forget_snapshots();
}
