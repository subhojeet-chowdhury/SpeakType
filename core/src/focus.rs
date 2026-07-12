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
