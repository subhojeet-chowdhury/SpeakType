use anyhow::Result;
use std::process::Command;

/// Returns a best-effort identifier for the currently focused window's
/// application (e.g. "code", "slack", "chrome", "terminal"). This feeds the
/// LLM cleanup step so it can pick a tone/format (Phase 4 in the plan).
///
/// Implementation note: there is no single cross-desktop-environment API for
/// "what app is focused" on Linux. This uses `xdotool` (X11) as the reference
/// implementation. On Wayland you'd swap this for a compositor-specific
/// protocol (e.g. wlr-foreign-toplevel-management) or a portal call — flagged
/// clearly here since it's the one part of this project that is genuinely
/// platform-fragile, not just platform-specific.
pub fn active_app_name() -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to get name of first application process whose frontmost is true"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_lowercase())
            }
            _ => Ok("unknown".to_string()),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("xdotool")
            .args(["getactivewindow", "getwindowclassname"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_lowercase())
            }
            _ => Ok("unknown".to_string()),
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        
        let mut pid = 0;
        unsafe {
            let hwnd = GetForegroundWindow();
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }

        if pid != 0 {
            let output = Command::new("cmd")
                .args(["/C", &format!("tasklist /FI \"PID eq {}\" /FO CSV /NH", pid)])
                .output();

            if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(first_quote) = stdout.find('"') {
                    if let Some(second_quote) = stdout[first_quote + 1..].find('"') {
                        let name = &stdout[first_quote + 1..first_quote + 1 + second_quote];
                        let name = name.to_lowercase().replace(".exe", "");
                        return Ok(name);
                    }
                }
            }
        }
        
        Ok("unknown".to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok("unknown".to_string())
    }
}
