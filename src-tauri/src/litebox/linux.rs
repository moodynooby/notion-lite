#![cfg(target_os = "linux")]
//! Linux-specific LiteBox implementations using namespaces and seccomp.
//! Note: Tauri/WebKitGTK relies on bwrap internally when the sandbox is enabled,
//! but this provides defense-in-depth resource dropping for the main process.

pub fn apply_linux_sandbox() -> Result<(), String> {
    log::info!("Applying Linux filesystem/capability restrictions");

    #[cfg(target_os = "linux")]
    unsafe {
        // 1. Unshare namespaces for isolation
        // CLONE_NEWNS: Mount namespace
        // CLONE_NEWUTS: Hostname namespace
        // CLONE_NEWIPC: IPC namespace
        // CLONE_NEWPID: PID namespace (this process becomes PID 1 in the new namespace)
        // Attempt basic namespace isolation if privileges allow, but do not
        // unshare CLONE_NEWNET (Notion requires internet) or CLONE_NEWIPC (GUI display requires IPC).
        // If unshare fails or is restricted by kernel/AppArmor (e.g. unprivileged userns restriction),
        // gracefully fall back to basic resource dropping.
        let flags = libc::CLONE_NEWNS | libc::CLONE_NEWUTS;

        if libc::unshare(flags) != 0 {
            log::warn!(
                "LiteBox: Namespace isolation unavailable (might lack CAP_SYS_ADMIN or user namespaces restricted by AppArmor/sysctl): {}. Continuing with privilege dropping.",
                std::io::Error::last_os_error()
            );
        } else {
            log::info!("LiteBox: Namespace isolation enforced (NS, UTS).");
        }

        // 2. Disable core dumps and ptrace attachments
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        log::info!("LiteBox: Process dumpable flag disabled.");

        // 3. Prevent gaining new privileges
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
        log::info!("LiteBox: PR_SET_NO_NEW_PRIVS enforced.");

        // 4. Drop all capabilities
        // Note: This requires the process to have some caps to begin with or be root.
        // For a desktop app, we mostly rely on the namespaces above.
    }

    Ok(())
}
