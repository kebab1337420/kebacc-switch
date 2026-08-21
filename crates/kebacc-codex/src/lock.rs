pub use kebacc_core::lock::{CRED_SWAP, MIDTASK, REFRESH, UPDATE, USAGE_CACHE};

pub fn locked<T>(name: &str, body: impl FnOnce() -> T) -> Result<T, String> {
    kebacc_core::lock::locked(&crate::provider::state_dir(), name, body)
}
