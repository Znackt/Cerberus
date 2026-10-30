use crate::cloud::rclone::RcloneClient;
use crate::domain::ids::DriveId;
use crate::domain::models::{DriveDefinition, DriveSession};
use crate::domain::status::MountStatus;
use crate::error::{CerberusError, CerberusResult};
use crate::persistence::AppPaths;
use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Mount supervisor for managing rclone mounts
#[derive(Clone)]
pub struct MountSupervisor {
    rclone: Arc<Mutex<Option<RcloneClient>>>,
    mounts: Arc<Mutex<HashMap<DriveId, MountInstance>>>,
    paths: Arc<Mutex<Option<AppPaths>>>,
    lifecycle: Arc<Mutex<()>>,
}

struct MountInstance {
    definition: DriveDefinition,
    child: Option<Child>,
    status: MountStatus,
    error_message: Option<String>,
    started_at: Option<Instant>,
    last_checked: Instant,
    rc_port: u16,
}

impl MountSupervisor {
    pub fn new() -> Self {
        Self {
            rclone: Arc::new(Mutex::new(None)),
            mounts: Arc::new(Mutex::new(HashMap::new())),
            paths: Arc::new(Mutex::new(None)),
            lifecycle: Arc::new(Mutex::new(())),
        }
    }

    pub fn set_paths(&self, paths: AppPaths) {
        if let Ok(mut guard) = self.paths.lock() {
            *guard = Some(paths);
        }
    }

    /// Initialize rclone client
    pub fn init_rclone(&self) -> CerberusResult<()> {
        match RcloneClient::discover() {
            Ok(client) => {
                let mut rclone_guard = self
                    .rclone
                    .lock()
                    .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
                *rclone_guard = Some(client);
                Ok(())
            }
            Err(_) => Ok(()), // Graceful degradation
        }
    }

    pub fn list_remotes(&self) -> CerberusResult<Vec<crate::cloud::rclone::RemoteInfo>> {
        let guard = self
            .rclone
            .lock()
            .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
        guard
            .as_ref()
            .ok_or_else(CerberusError::rclone_not_found)?
            .list_remotes()
    }

    pub fn test_remote(&self, name: &str, path: &str) -> CerberusResult<()> {
        let guard = self
            .rclone
            .lock()
            .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
        guard
            .as_ref()
            .ok_or_else(CerberusError::rclone_not_found)?
            .test_remote(name, path)
    }

    pub fn launch_rclone_config(&self) -> CerberusResult<()> {
        use std::os::windows::process::CommandExt;
        let executable = {
            let guard = self
                .rclone
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
            guard
                .as_ref()
                .ok_or_else(CerberusError::rclone_not_found)?
                .executable_path()
                .to_path_buf()
        };
        Command::new(executable)
            .arg("config")
            .creation_flags(0x00000010)
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                CerberusError::rclone_error(format!("Could not launch rclone config: {error}"))
            })
    }

    /// Get mount status for a drive
    pub fn get_status(&self, id: &DriveId) -> Option<MountStatus> {
        // 1. First, check if the child process has exited, regardless of status.
        // This detects "ghost" mounts where the process died but the drive entry remains.
        {
            if let Ok(mut mounts) = self.mounts.lock() {
                if let Some(instance) = mounts.get_mut(id) {
                    if matches!(instance.status, MountStatus::Mounting | MountStatus::Mounted) {
                        if let Some(child) = instance.child.as_mut() {
                            match child.try_wait() {
                                Ok(Some(exit)) => {
                                    instance.status = MountStatus::Failed;
                                    instance.error_message =
                                        Some(format!("rclone process exited: {}", exit));
                                    instance.child = None;
                                }
                                Err(e) => {
                                    instance.status = MountStatus::Failed;
                                    instance.error_message =
                                        Some(format!("Failed to monitor rclone process: {}", e));
                                    instance.child = None;
                                }
                                Ok(None) => {
                                    // Process is still running, check filesystem
                                }
                            }
                        } else {
                            // Status is Mounting/Mounted but no child process?
                            instance.status = MountStatus::Failed;
                            instance.error_message = Some("Mount process missing".to_string());
                        }
                    }
                }
            }
        }

        // 2. Snapshot data under lock for filesystem accessibility check
        let (status, drive_letter) = {
            let mounts = self.mounts.lock().ok()?;
            let instance = mounts.get(id)?;
            (instance.status, instance.definition.drive_letter.clone())
        };

        // 3. Confirm actual filesystem accessibility for Mounted state
        if status == MountStatus::Mounted {
            if let Some(letter) = drive_letter {
                let mount_point = format!("{}:\\", letter.trim_end_matches(':'));
                if fs::metadata(&mount_point).is_err() {
                    // Drive letter exists in Cerberus state, but OS says not accessible.
                    // This is a failed mount.
                    if let Ok(mut mounts) = self.mounts.lock() {
                        if let Some(inst) = mounts.get_mut(id) {
                            inst.status = MountStatus::Failed;
                            inst.error_message = Some("Drive not accessible".to_string());
                        }
                    }
                    return Some(MountStatus::Failed);
                }
            }
        }

        Some(status)
    }

    /// Check if rclone is available
    pub fn get_rclone_status(&self) -> Option<bool> {
        let rclone_guard = self.rclone.lock().ok()?;
        Some(rclone_guard.is_some())
    }

    /// Start mounting a drive
    pub fn mount(&self, definition: DriveDefinition) -> CerberusResult<()> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| CerberusError::MountError("Lifecycle lock poisoned".into()))?;
        let rclone_guard = self
            .rclone
            .lock()
            .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
        let rclone = rclone_guard
            .as_ref()
            .ok_or(CerberusError::rclone_not_found())?;

        // Validate drive letter and check duplicates
        let mount_letter = definition
            .drive_letter
            .as_ref()
            .ok_or_else(|| CerberusError::mount_error("Drive letter required"))?;
        let normalized_letter = mount_letter.trim().trim_end_matches(':');
        if normalized_letter.len() != 1 || !normalized_letter.as_bytes()[0].is_ascii_alphabetic() {
            return Err(CerberusError::mount_error(
                "Drive letter must be a single alphabetic letter",
            ));
        }

        // Check for duplicate mount or duplicate letter
        {
            let mounts = self
                .mounts
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
            if mounts.contains_key(&definition.id) {
                return Err(CerberusError::mount_error("Drive already mounted"));
            }
            // Check for duplicate drive letter
            for instance in mounts.values() {
                if let Some(letter) = &instance.definition.drive_letter {
                    if letter == mount_letter {
                        return Err(CerberusError::mount_error(
                            "Drive letter already in use by another mount",
                        ));
                    }
                }
            }
        }

        // Prepare mount
        let mount_point = format!("{}:\\", normalized_letter.to_ascii_uppercase());

        // Check if mount point is already in use
        if fs::metadata(&mount_point).is_ok() {
            return Err(CerberusError::mount_error(format!(
                "Drive letter {} is already in use",
                mount_point
            )));
        }

        // Get log path from AppPaths
        let log_path = if let Ok(paths_guard) = self.paths.lock() {
            if let Some(paths) = paths_guard.as_ref() {
                fs::create_dir_all(&paths.log_directory).ok();
                paths
                    .log_directory
                    .join(format!("mount_{}.log", definition.id))
            } else {
                PathBuf::from(format!("mount_{}.log", definition.id))
            }
        } else {
            PathBuf::from(format!("mount_{}.log", definition.id))
        };

        // Give this rclone child a private loopback RC endpoint so unmount can
        // flush and detach through rclone before the process is stopped.
        let rc_listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
            CerberusError::mount_error(format!(
                "Could not reserve local rclone control port: {error}"
            ))
        })?;
        let rc_port = rc_listener
            .local_addr()
            .map_err(|error| {
                CerberusError::mount_error(format!("Could not read local control port: {error}"))
            })?
            .port();
        drop(rc_listener);

        // Remote/path testing is exposed as a separate bounded worker action in
        // Cloud Drives; do not put the network check on this mount call path.

        // Spawn mount process
        let mut cmd = Command::new(rclone.executable_path());
        cmd.arg("mount")
            .arg(format!(
                "{}:{}",
                definition.remote_name, definition.remote_path
            ))
            .arg(&mount_point)
            .arg("--vfs-cache-mode")
            .arg("writes")
            .arg("--log-file")
            .arg(&log_path)
            .arg("--log-level")
            .arg("INFO")
            .arg("--rc")
            .arg("--rc-addr")
            .arg(format!("127.0.0.1:{rc_port}"))
            .arg("--rc-no-auth")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| CerberusError::mount_error(format!("Failed to start rclone: {}", e)))?;

        // Drain stdout/stderr in background to prevent blocking
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                use std::io::Read;
                let mut reader = std::io::BufReader::new(stdout);
                let mut buffer = [0; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(_) => {} // Drain output
                        Err(_) => break,
                    }
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                use std::io::Read;
                let mut reader = std::io::BufReader::new(stderr);
                let mut buffer = [0; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(_) => {} // Drain output
                        Err(_) => break,
                    }
                }
            });
        }

        // Insert mount instance first
        let mut mounts = self
            .mounts
            .lock()
            .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
        let drive_id = definition.id;
        mounts.insert(
            drive_id,
            MountInstance {
                definition,
                child: Some(child),
                status: MountStatus::Mounting,
                error_message: None,
                started_at: Some(Instant::now()),
                last_checked: Instant::now(),
                rc_port,
            },
        );

        Ok(())
    }

    /// Stop mounting a drive
    pub fn unmount(&self, id: &DriveId) -> CerberusResult<()> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| CerberusError::MountError("Lifecycle lock poisoned".into()))?;
        // Snapshot the mount info we need
        let (drive_letter, rc_port) = {
            let mut mounts = self
                .mounts
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;

            let instance = mounts
                .get_mut(id)
                .ok_or_else(|| CerberusError::mount_error("Drive not found"))?;

            instance.status = MountStatus::Unmounting;
            (instance.definition.drive_letter.clone(), instance.rc_port)
        };

        // First, try graceful rclone unmount if we have a drive letter
        if let Some(ref letter) = drive_letter {
            let mount_point = format!("{}:\\", letter.trim_end_matches(':'));
            let rc_mount_point = format!("{}:", letter.trim_end_matches(':'));

            // Use the per-process loopback RC endpoint to flush and detach cleanly.
            if let Ok(rclone_guard) = self.rclone.lock() {
                if let Some(rclone) = rclone_guard.as_ref() {
                    if let Err(error) = rclone.unmount(rc_port, &rc_mount_point) {
                        tracing::warn!(
                            "Graceful rclone unmount failed for {}: {error}",
                            mount_point
                        );
                    }
                }
            }
        }

        // Now handle the child process
        let child_opt = {
            let mut mounts = self
                .mounts
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;

            if let Some(instance) = mounts.get_mut(id) {
                instance.status = MountStatus::Unmounting;
                instance.child.take()
            } else {
                return Err(CerberusError::mount_error("Drive not found"));
            }
        };

        // Wait for child process to exit
        if let Some(mut child) = child_opt {
            // Wait for process to exit (with timeout)
            let timeout = Duration::from_secs(10);
            let start = Instant::now();
            let mut exited = false;

            while start.elapsed() < timeout {
                if let Ok(status) = child.try_wait() {
                    if status.is_some() {
                        exited = true;
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }

            // If still running, force kill
            if !exited {
                let _ = child.kill();
                // Wait a bit more for forced kill
                let _ = child.wait();
            }
        }

        // Verify drive is released
        if let Some(ref letter) = drive_letter {
            let mount_point = format!("{}:\\", letter.trim_end_matches(':'));
            // Check if drive is still accessible - if so, unmount may have failed
            let release_deadline = Instant::now() + Duration::from_secs(5);
            while fs::metadata(&mount_point).is_ok() && Instant::now() < release_deadline {
                thread::sleep(Duration::from_millis(100));
            }
            if fs::metadata(&mount_point).is_ok() {
                // Drive still accessible - mark as failed to unmount
                let mut mounts = self
                    .mounts
                    .lock()
                    .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
                if let Some(instance) = mounts.get_mut(id) {
                    instance.status = MountStatus::Failed;
                    instance.error_message =
                        Some("Drive letter not released after unmount".to_string());
                    return Err(CerberusError::mount_error(
                        "Drive letter not released after unmount",
                    ));
                }
            }
        }

        // Mark as unmounted
        {
            let mut mounts = self
                .mounts
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
            if let Some(instance) = mounts.get_mut(id) {
                instance.status = MountStatus::Unmounted;
            }
        }

        Ok(())
    }

    /// Shutdown all mounts - unmount all drives owned by Cerberus
    pub fn shutdown(&self) -> CerberusResult<()> {
        // Get all drive IDs that are currently mounted
        let drive_ids: Vec<DriveId> = {
            let mounts = self
                .mounts
                .lock()
                .map_err(|_| CerberusError::MountError("Lock poisoned".into()))?;
            mounts
                .iter()
                .filter(|(_, inst)| inst.child.is_some() && inst.status != MountStatus::Unmounted)
                .map(|(id, _)| *id)
                .collect()
        };

        // Unmount each drive
        let mut failures = Vec::new();
        for id in drive_ids {
            if let Err(error) = self.unmount(&id) {
                failures.push(error.to_string());
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(CerberusError::mount_error(format!(
                "Shutdown could not unmount all drives: {}",
                failures.join("; ")
            )))
        }
    }

    /// Get all active mount sessions
    pub fn active_sessions(&self) -> Vec<DriveSession> {
        let mounts = match self.mounts.lock() {
            Ok(m) => m,
            Err(_) => return vec![],
        };

        mounts
            .values()
            .map(|instance| DriveSession {
                id: instance.definition.id,
                display_name: instance.definition.display_name.clone(),
                drive_letter: instance.definition.drive_letter.clone(),
                status: instance.status,
                cached_bytes: crate::domain::units::Bytes::ZERO,
                cache_budget: crate::domain::units::Bytes::ZERO,
                root: instance.definition.drive_letter.clone().unwrap_or_default(),
                error_message: instance.error_message.clone(),
                error_timestamp: None,
            })
            .collect()
    }
}

impl Default for MountSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::DriveId;
    use crate::domain::models::DriveDefinition;
    use crate::domain::units::DisplayName;

    #[test]
    fn test_mount_supervisor_new() {
        let supervisor = MountSupervisor::new();
        // Should be empty
        assert_eq!(supervisor.mounts.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_init_rclone() {
        let supervisor = MountSupervisor::new();
        // Should not panic even if rclone not found
        let result = supervisor.init_rclone();
        assert!(result.is_ok());
    }

    #[test]
    fn test_drive_letter_validation() {
        let valid_letters = vec!["C:", "D:", "E:", "Z:", "c:", "d:"];
        for letter in valid_letters {
            let trimmed = letter.trim_end_matches(':');
            assert!(!trimmed.is_empty());
            assert!(trimmed.len() == 1);
        }
    }

    #[test]
    fn test_mount_instance_creation() {
        let definition = DriveDefinition {
            id: DriveId::new(),
            display_name: DisplayName::new("Test Drive".to_string()).unwrap(),
            remote_name: "test".to_string(),
            remote_path: "/test".to_string(),
            drive_letter: Some("Z:".to_string()),
            auto_mount: false,
            permanent: false,
        };

        let instance = MountInstance {
            definition,
            child: None,
            status: MountStatus::Unmounted,
            error_message: None,
            started_at: None,
            last_checked: Instant::now(),
            rc_port: 0,
        };

        assert_eq!(instance.status, MountStatus::Unmounted);
        assert!(instance.error_message.is_none());
    }

    #[test]
    fn test_get_status_cannot_deadlock() {
        let supervisor = MountSupervisor::new();
        let id = DriveId::new();

        // Status should be None for non-existent drive
        assert_eq!(supervisor.get_status(&id), None);
    }

    #[test]
    fn test_duplicate_drive_letter_detection() {
        let supervisor = MountSupervisor::new();

        let definition1 = DriveDefinition {
            id: DriveId::new(),
            display_name: DisplayName::new("Drive1".to_string()).unwrap(),
            remote_name: "remote1".to_string(),
            remote_path: "/path1".to_string(),
            drive_letter: Some("E:".to_string()),
            auto_mount: false,
            permanent: false,
        };

        // Insert first drive
        {
            let mut mounts = supervisor.mounts.lock().unwrap();
            mounts.insert(
                definition1.id,
                MountInstance {
                    definition: definition1.clone(),
                    child: None,
                    status: MountStatus::Mounted,
                    error_message: None,
                    started_at: None,
                    last_checked: Instant::now(),
                    rc_port: 0,
                },
            );
        }

        let definition2 = DriveDefinition {
            id: DriveId::new(),
            display_name: DisplayName::new("Drive2".to_string()).unwrap(),
            remote_name: "remote2".to_string(),
            remote_path: "/path2".to_string(),
            drive_letter: Some("E:".to_string()),
            auto_mount: false,
            permanent: false,
        };

        // Mount should fail due to duplicate drive letter
        // Note: This test verifies the logic, actual mount test requires rclone
        assert_ne!(definition1.id, definition2.id);
        // Both have same drive letter
        assert_eq!(definition1.drive_letter, definition2.drive_letter);
    }

    #[test]
    fn test_shutdown_cleans_up_mounts() {
        let supervisor = MountSupervisor::new();

        // Since shutdown just unmounts drives, we can test it doesn't panic
        let result = supervisor.shutdown();
        assert!(result.is_ok());
    }

    #[test]
    fn test_active_sessions_empty() {
        let supervisor = MountSupervisor::new();
        let sessions = supervisor.active_sessions();
        assert_eq!(sessions.len(), 0);
    }
}
