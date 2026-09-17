/// Spawns a fire-and-forget child process. On Unix the child is reaped by a
/// short-lived thread once it exits so no zombie accumulates; on Windows the
/// process handle is released when the `Child` drops.
pub fn spawn_detached(mut command: std::process::Command) -> std::io::Result<()> {
    #[cfg(not(windows))]
    {
        let mut child = command.spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    #[cfg(windows)]
    {
        command.spawn()?;
    }
    Ok(())
}
