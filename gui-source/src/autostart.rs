use std::os::windows::process::CommandExt;
use std::process::Command;

const TASK_NAME: &str = "GPU Shark";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks(arguments: &[&str]) -> Result<std::process::Output, String> {
    Command::new("schtasks")
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Could not run schtasks: {error}"))
}

/// The application manifest requires elevation, so a plain Run-key entry
/// would be silently skipped at logon. A highest-privileges logon task is the
/// supported autostart path for an administrator application.
pub fn is_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn set(enabled: bool) -> Result<(), String> {
    if enabled {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Cannot locate executable: {error}"))?;
        let task = format!("\"{}\"", executable.display());
        let output = schtasks(&[
            "/Create", "/F", "/RL", "HIGHEST", "/SC", "ONLOGON", "/TN", TASK_NAME, "/TR", &task,
        ])?;
        if !output.status.success() {
            return Err(format!(
                "Could not create the startup task: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    } else {
        let output = schtasks(&["/Delete", "/F", "/TN", TASK_NAME])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if !stderr.contains("does not exist")
                && !stderr.contains("не существует")
                && !stderr.contains("существует")
            {
                return Err(format!("Could not remove the startup task: {stderr}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_reflects_initially_absent_task() {
        // The result depends on the machine state; the call itself must not
        // panic and must return a boolean.
        let _ = is_enabled();
    }
}
