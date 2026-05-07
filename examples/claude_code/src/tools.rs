use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, OnceLock},
    thread,
    time::Duration,
};

static WORKDIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_workdir(path: PathBuf) {
    WORKDIR.set(path).ok();
}

pub fn get_workdir() -> &'static PathBuf {
    WORKDIR.get_or_init(|| std::env::current_dir().expect("Failed to get current working directory"))
}

pub fn safe_path<P: AsRef<Path>>(p: P) -> anyhow::Result<PathBuf> {
    let workdir = get_workdir();
    let p = p.as_ref();

    let path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    };

    let path = path.canonicalize()?;

    if !path.starts_with(workdir) {
        anyhow::bail!("Path escapes workspace: {:?}", p);
    }

    Ok(path)
}

/// Run a shell command with 120s timeout.
#[abu_macros::tool(
    struct_name = Bash,
    description = "Run a shell command in the workspace. Use for file operations, git, building, testing, etc."
)]
pub fn run_bash(command: &str) -> String {
    let dangerous = ["rm -rf /", "sudo", "shutdown", "reboot", "> /dev/"];
    if dangerous.iter().any(|&d| command.contains(d)) {
        return "Error: Dangerous command blocked".to_string();
    }

    let cmd_str = command.to_string();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd_str)
            .current_dir(get_workdir())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(output);
    });

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(output)) => {
            let mut out = String::from_utf8_lossy(&output.stdout).into_owned();
            let err = String::from_utf8_lossy(&output.stderr);
            if !err.is_empty() {
                out.push('\n');
                out.push_str(&err);
            }
            let out = out.trim().to_string();
            if out.is_empty() {
                "(no output)".to_string()
            } else {
                out.chars().take(50000).collect()
            }
        }
        Ok(Err(e)) => format!("Error: {}", e),
        Err(_) => "Error: Timeout (120s)".to_string(),
    }
}

/// Read file contents.
#[abu_macros::tool(
    struct_name = ReadFile,
    description = "Read the contents of a file. Returns the full file content.",
    category = "safe"
)]
pub fn run_read(path: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    match std::fs::read_to_string(&fp) {
        Ok(t) => t,
        Err(e) => format!("Error: {}", e),
    }
}

/// Write content to a file, creating parent directories as needed.
#[abu_macros::tool(
    struct_name = WriteFile,
    description = "Write content to a file. Creates parent directories automatically. Overwrites existing files."
)]
pub fn run_write(path: &str, content: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    if let Some(parent) = fp.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("Error: {}", e);
        }
    }

    match std::fs::write(&fp, content) {
        Ok(_) => format!("Wrote {} bytes to {}", content.len(), path),
        Err(e) => format!("Error: {}", e),
    }
}

/// Edit a file by replacing one string with another.
#[abu_macros::tool(
    struct_name = EditFile,
    description = "Edit a file by performing exact string replacement. The old_string must match exactly one occurrence in the file."
)]
pub fn run_edit(path: &str, old_string: &str, new_string: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    let content = match std::fs::read_to_string(&fp) {
        Ok(c) => c,
        Err(e) => return format!("Error reading file: {}", e),
    };

    let count = content.matches(old_string).count();
    if count == 0 {
        return format!("Error: old_string not found in file");
    }
    if count > 1 {
        return format!(
            "Error: old_string matches {} occurrences. Must match exactly one.",
            count
        );
    }

    let new_content = content.replacen(old_string, new_string, 1);
    match std::fs::write(&fp, &new_content) {
        Ok(_) => format!("Successfully edited {}", path),
        Err(e) => format!("Error writing file: {}", e),
    }
}
