use crate::error::CerberusResult;
use std::path::Path;
use std::process::Command;

/// Status of WinFsp detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinFspStatus {
    Available,
    NotFound,
}

/// Checker for WinFsp availability
pub struct WinFspChecker;

impl WinFspChecker {
    /// Check if WinFsp is available on the system
    pub fn check() -> CerberusResult<WinFspStatus> {
        // Check for WinFsp DLLs in common locations
        let candidates = vec![
            "C:\\Windows\\System32\\WinFsp.dll",
            "C:\\Windows\\SysWOW64\\WinFsp.dll",
            "C:\\Windows\\System32\\winfsp-x64.dll",
            "C:\\Windows\\SysWOW64\\winfsp-x86.dll",
            "C:\\Program Files\\WinFsp\\bin\\WinFsp.dll",
            "C:\\Program Files (x86)\\WinFsp\\bin\\WinFsp.dll",
            "C:\\Program Files\\WinFsp\\SxS\\*\\bin\\winfsp-x64.dll",
        ];

        for candidate in candidates {
            if Path::new(candidate).exists() {
                return Ok(WinFspStatus::Available);
            }
            // Check glob pattern for SxS
            if candidate.contains('*') {
                if let Ok(entries) = std::fs::read_dir("C:\\Program Files (x86)\\WinFsp\\SxS") {
                    for entry in entries.flatten() {
                        let dll_path = entry.path().join("bin").join("winfsp-x64.dll");
                        if dll_path.exists() {
                            return Ok(WinFspStatus::Available);
                        }
                    }
                }
            }
        }

        // Check via registry using Windows API
        if Self::check_registry() {
            return Ok(WinFspStatus::Available);
        }

        // Check if WinFsp.Launcher service is running
        if Self::check_service() {
            return Ok(WinFspStatus::Available);
        }

        Ok(WinFspStatus::NotFound)
    }

    fn check_registry() -> bool {
        // Check HKLM\SOFTWARE\WinFsp for installation
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::winnt::{KEY_READ, LPCWSTR};
        use winapi::um::winreg::{RegCloseKey, RegOpenKeyExW, HKEY_LOCAL_MACHINE};

        let key_name: Vec<u16> = OsStr::new("SOFTWARE\\WinFsp")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey = std::ptr::null_mut();
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                key_name.as_ptr() as LPCWSTR,
                0,
                KEY_READ,
                &mut hkey,
            )
        };

        if result == 0 {
            unsafe { RegCloseKey(hkey) };
            return true;
        }

        // Also check HKLM\SOFTWARE\WOW6432Node\WinFsp for 32-bit on 64-bit
        let key_name_wow: Vec<u16> = OsStr::new("SOFTWARE\\WOW6432Node\\WinFsp")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey_wow = std::ptr::null_mut();
        let result_wow = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                key_name_wow.as_ptr() as LPCWSTR,
                0,
                KEY_READ,
                &mut hkey_wow,
            )
        };

        if result_wow == 0 {
            unsafe { RegCloseKey(hkey_wow) };
            return true;
        }

        false
    }

    fn check_service() -> bool {
        // Check if WinFsp.Launcher service exists and is running
        let output = Command::new("sc")
            .arg("query")
            .arg("WinFsp.Launcher")
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Service is running if output contains "RUNNING"
            if stdout.contains("RUNNING") || stdout.contains("STOPPED") {
                // Service exists (even if stopped, it means installed)
                return true;
            }
        }

        false
    }

    /// Get installation instructions for WinFsp
    pub fn installation_instructions() -> &'static str {
        "WinFsp is required for mounting. Download from https://winfsp.dev/"
    }
}
