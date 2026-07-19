use anyhow::Result;

pub fn active_app_name() -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSRunningApplication, NSWorkspace};
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let app = unsafe { workspace.frontmostApplication() };
        if let Some(app) = app {
            if let Some(name) = unsafe { app.localizedName() } {
                return Ok(name.to_string().to_lowercase());
            }
        }
        Ok("unknown".to_string())
    }

    #[cfg(target_os = "linux")]
    {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
        
        if let Ok((conn, screen_num)) = x11rb::connect(None) {
            let screen = &conn.setup().roots[screen_num];
            if let Ok(active_window_atom) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
                if let Ok(reply) = active_window_atom.reply() {
                    if let Ok(prop) = conn.get_property(false, screen.root, reply.atom, AtomEnum::WINDOW, 0, 1) {
                        if let Ok(reply) = prop.reply() {
                            if reply.value.len() >= 4 {
                                let mut bytes = [0u8; 4];
                                bytes.copy_from_slice(&reply.value[0..4]);
                                let window_id = u32::from_ne_bytes(bytes);
                                
                                if let Ok(class_prop) = conn.get_property(false, window_id, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024) {
                                    if let Ok(reply) = class_prop.reply() {
                                        let mut iter = reply.value.split(|&b| b == 0);
                                        if let Some(instance) = iter.next() {
                                            return Ok(String::from_utf8_lossy(instance).to_lowercase());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok("unknown".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
        use windows::Win32::Foundation::MAX_PATH;

        let mut pid = 0;
        unsafe {
            let hwnd = GetForegroundWindow();
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            if pid != 0 {
                if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let mut path = [0u16; MAX_PATH as usize];
                    let len = GetModuleFileNameExW(handle, None, &mut path);
                    if len > 0 {
                        let path_str = String::from_utf16_lossy(&path[..len as usize]);
                        let name = std::path::Path::new(&path_str)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_lowercase();
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
