use crate::error::{CerberusError, CerberusResult};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Status of rclone detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneStatus {
    Available,
    NotFound,
    InvalidVersion,
    ConfigLocked,
}

/// Information about a rclone remote
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    pub name: String,
    pub display_name: String,
}

/// rclone client for operations
pub struct RcloneClient {
    executable_path: PathBuf,
    version: Option<String>,
}

impl RcloneClient {
    /// Discover rclone on PATH or return error
    pub fn discover() -> CerberusResult<Self> {
        // Try common locations first
        let candidates = vec![
            "rclone.exe",
            "C:\\Program Files\\rclone\\rclone.exe",
            "C:\\Program Files (x86)\\rclone\\rclone.exe",
        ];

        for candidate in candidates {
            if let Some(path) = Self::find_executable(candidate) {
                if let Some(version) = Self::get_version(&path)? {
                    return Ok(Self {
                        executable_path: path,
                        version: Some(version),
                    });
                }
            }
        }

        Err(CerberusError::rclone_not_found())
    }

    /// Create client with explicit path
    pub fn with_path(path: PathBuf) -> CerberusResult<Self> {
        if !path.exists() {
            return Err(CerberusError::rclone_not_found());
        }

        let version = Self::get_version(&path)?;
        Ok(Self {
            executable_path: path,
            version,
        })
    }

    fn find_executable(name: &str) -> Option<PathBuf> {
        let path = PathBuf::from(name);
        if path.exists() {
            Some(path)
        } else {
            // Try PATH
            if let Ok(paths) = std::env::var("PATH") {
                for p in paths.split(';') {
                    let candidate = Path::new(p).join(name);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
            None
        }
    }

    fn get_version(path: &Path) -> CerberusResult<Option<String>> {
        let output = Command::new(path)
            .arg("version")
            .output()
            .map_err(|_| CerberusError::rclone_not_found())?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse version from output like "rclone v1.67.0"
        if let Some(line) = stdout.lines().next() {
            if line.starts_with("rclone") {
                return Ok(Some(line.to_string()));
            }
        }
        Ok(None)
    }

    /// List configured remotes (names only, no credentials)
    pub fn list_remotes(&self) -> CerberusResult<Vec<RemoteInfo>> {
        let output = Self::run_bounded(
            &self.executable_path,
            &["listremotes"],
            Duration::from_secs(15),
        )
        .map_err(|e| CerberusError::rclone_error(format!("Failed to run rclone: {}", e)))?;

        if !output.status.success() {
            return Err(CerberusError::rclone_error("rclone listremotes failed"));
        }

        Ok(parse_remote_names(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Test remote connectivity
    pub fn test_remote(&self, remote_name: &str, remote_path: &str) -> CerberusResult<()> {
        let remote = format!("{}:{}", remote_name, remote_path);

        let output = Self::run_bounded(
            &self.executable_path,
            &["ls", &remote, "--max-depth", "1"],
            Duration::from_secs(30),
        )
        .map_err(|e| CerberusError::rclone_error(format!("Failed to test remote: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(CerberusError::rclone_error(format!("Remote test failed (rclone exit code {:?}); check remote configuration and permissions", output.status.code())))
        }
    }

    /// Ask a running rclone process to unmount its drive through its loopback-only RC API.
    pub fn unmount(&self, rc_port: u16, mount_point: &str) -> CerberusResult<()> {
        let url = format!("http://127.0.0.1:{rc_port}");
        let mount_arg = format!("mountPoint={mount_point}");
        let result = Self::run_bounded(
            &self.executable_path,
            &["rc", "--url", &url, "mount/unmount", &mount_arg],
            Duration::from_secs(10),
        )
        .map_err(|error| {
            CerberusError::rclone_error(format!("Graceful unmount request failed: {error}"))
        })?;
        if !result.status.success() {
            return Err(CerberusError::rclone_error(format!(
                "Graceful unmount request returned exit code {:?}",
                result.status.code()
            )));
        }

        // The mount is now flushed and detached. Ask the per-mount rclone process
        // to exit; if it already exited, the supervisor's child wait handles that.
        let _ = Self::run_bounded(
            &self.executable_path,
            &["rc", "--url", &url, "core/quit"],
            Duration::from_secs(3),
        );
        Ok(())
    }

    fn run_bounded(path: &Path, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
        let mut child = Command::new(path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let stdout_thread = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let stderr_thread = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "rclone command timed out",
                ));
            }
            thread::sleep(Duration::from_millis(25));
        };
        Ok(Output {
            status,
            stdout: stdout_thread.join().unwrap_or_default(),
            stderr: stderr_thread.join().unwrap_or_default(),
        })
    }

    /// Get rclone version
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Get executable path
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }
}

fn parse_remote_names(output: &str) -> Vec<RemoteInfo> {
    output
        .lines()
        .filter_map(|line| {
            let name = line.trim().strip_suffix(':').unwrap_or(line.trim()).trim();
            (!name.is_empty()).then(|| RemoteInfo {
                name: name.to_owned(),
                display_name: name.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_remote_names_from_rclone_output() {
        let remotes = parse_remote_names("drive:\r\narchive:\n\n  backup:\n");
        assert_eq!(
            remotes
                .iter()
                .map(|remote| remote.name.as_str())
                .collect::<Vec<_>>(),
            ["drive", "archive", "backup"]
        );
    }

    #[test]
    fn empty_remote_output_is_empty() {
        assert!(parse_remote_names("\n  \n").is_empty());
    }
}
