use crate::litebox::LiteBox;

/// Applies the OS-level sandbox boundaries at startup.
///
/// Panics if the sandbox cannot be applied: Zero-Trust is the default, and the
/// app should not run without its defense-in-depth layer.
pub fn init() {
    let mut sandbox = LiteBox::new();
    if let Err(e) = sandbox.apply_sandbox() {
        log::error!("CRITICAL: Failed to apply OS-level LiteBox sandbox: {}", e);
        panic!("Security sandbox initialization failed: {}", e);
    }
}
