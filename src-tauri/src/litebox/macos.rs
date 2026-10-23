#![cfg(target_os = "macos")]
//! macOS-specific LiteBox implementations using process security limits.

pub fn apply_macos_sandbox() -> Result<(), String> {
    log::info!("Applying macOS process boundary restrictions");

    #[cfg(target_os = "macos")]
    unsafe {
        // 1. Prevent process tracing/debugging attach (PT_DENY_ATTACH = 31)
        // Note: PT_DENY_ATTACH is 31 on macOS
        const PT_DENY_ATTACH: libc::c_int = 31;
        if libc::ptrace(PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0) != 0 {
            log::warn!("LiteBox: PT_DENY_ATTACH failed or process is already being debugged.");
        } else {
            log::info!("LiteBox: PT_DENY_ATTACH enforced (debugging prevented).");
        }

        // 2. Disable core dump generation
        let rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &rlim) != 0 {
            log::warn!("LiteBox: Failed to disable core dump generation.");
        } else {
            log::info!("LiteBox: Core dump size limit set to 0.");
        }
    }

    Ok(())
}
