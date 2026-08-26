use std::path::PathBuf;
use std::process::Command;

/// Enrich PATH environment variable on macOS/Linux with architecture-appropriate paths.
/// Especially on macOS, GUI applications (launched from Finder / Dock) have a minimal PATH
/// (/usr/bin:/bin:/usr/sbin:/sbin) and miss Homebrew / MacPorts / user binaries.
pub fn ensure_path_env() {
    #[cfg(unix)]
    {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let mut paths: Vec<PathBuf> = std::env::split_paths(&current_path).collect();

        let mut candidate_paths: Vec<PathBuf> = Vec::new();

        // 1. Current executable directory (e.g. VideoCropTrim.app/Contents/MacOS)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidate_paths.push(exe_dir.to_path_buf());
            }
        }

        // 2. Architecture-specific package manager paths
        #[cfg(target_os = "macos")]
        {
            let arch = std::env::consts::ARCH;
            if arch == "aarch64" {
                // Apple Silicon Homebrew priority
                candidate_paths.push(PathBuf::from("/opt/homebrew/bin"));
                candidate_paths.push(PathBuf::from("/opt/homebrew/sbin"));
                candidate_paths.push(PathBuf::from("/usr/local/bin"));
                candidate_paths.push(PathBuf::from("/usr/local/sbin"));
            } else {
                // Intel Mac Homebrew priority
                candidate_paths.push(PathBuf::from("/usr/local/bin"));
                candidate_paths.push(PathBuf::from("/usr/local/sbin"));
                candidate_paths.push(PathBuf::from("/opt/homebrew/bin"));
                candidate_paths.push(PathBuf::from("/opt/homebrew/sbin"));
            }
            // MacPorts and other common Unix paths
            candidate_paths.push(PathBuf::from("/opt/local/bin"));
            candidate_paths.push(PathBuf::from("/opt/local/sbin"));
        }

        #[cfg(not(target_os = "macos"))]
        {
            candidate_paths.push(PathBuf::from("/usr/local/bin"));
            candidate_paths.push(PathBuf::from("/usr/local/sbin"));
            candidate_paths.push(PathBuf::from("/usr/bin"));
        }

        // 3. User home bin paths
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidate_paths.push(home_path.join(".local/bin"));
            candidate_paths.push(home_path.join("bin"));
            candidate_paths.push(home_path.join(".cargo/bin"));
        }

        // Prepend candidate paths that exist and are not already in PATH
        for cand in candidate_paths.into_iter().rev() {
            if cand.exists() && !paths.iter().any(|p| p == &cand) {
                paths.insert(0, cand);
            }
        }

        if let Ok(new_path) = std::env::join_paths(paths) {
            std::env::set_var("PATH", new_path);
        }
    }
}

/// Check if ffmpeg binary is available and executable.
pub fn is_ffmpeg_installed() -> bool {
    create_hidden_command("ffmpeg")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Check if ffprobe binary is available and executable.
pub fn is_ffprobe_installed() -> bool {
    create_hidden_command("ffprobe")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Create a Command that runs silently in the background without creating or flashing a Windows console window.
pub fn create_hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
    }
    cmd
}

