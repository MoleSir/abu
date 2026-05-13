use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, OnceLock},
    thread,
    time::Duration,
};

static WORKDIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_workdir(path: PathBuf) {
    // Canonicalize the workdir so symlink-comparison is correct.
    let canonical = path.canonicalize().unwrap_or(path);
    WORKDIR.set(canonical).ok();
}

pub fn get_workdir() -> &'static PathBuf {
    WORKDIR.get_or_init(|| {
        let cwd = std::env::current_dir().expect("Failed to get current working directory");
        cwd.canonicalize().unwrap_or(cwd)
    })
}

/// Resolve and validate a path that MUST already exist (for Read, Edit, etc.).
///
/// Uses `Path::strip_prefix` (component-boundary aware) rather than
/// `str::starts_with` (string prefix) to prevent path traversal bypass
/// (CVE-2025-54794 class vulnerability).
pub fn safe_path<P: AsRef<Path>>(p: P) -> anyhow::Result<PathBuf> {
    let workdir = get_workdir();
    let p = p.as_ref();

    let path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    };

    let path = path.canonicalize()?;

    if path.strip_prefix(workdir).is_err() {
        anyhow::bail!("Path escapes workspace: {:?}", p);
    }

    Ok(path)
}

/// Resolve and validate a path that does NOT need to exist yet (for WriteFile).
/// Only the deepest existing ancestor is canonicalized; the non-existent tail is
/// verified to not escape the workspace via `..` traversal.
pub fn safe_new_path(p: &str) -> anyhow::Result<PathBuf> {
    let workdir = get_workdir();
    let candidate = if std::path::Path::new(p).is_absolute() {
        std::path::PathBuf::from(p)
    } else {
        workdir.join(p)
    };

    // Walk up to the first existing ancestor and canonicalize it
    let (existing_ancestor, trailing) = find_existing_ancestor(&candidate);
    let canonical_ancestor = existing_ancestor.canonicalize()?;

    let resolved = canonical_ancestor.join(&trailing);
    let normalized = normalize_path(&resolved);

    // Path-component-aware boundary check: strip_prefix prevents
    // /project-evil from matching the boundary of /project
    if normalized.strip_prefix(workdir).is_err() {
        anyhow::bail!("Path escapes workspace: {:?}", p);
    }

    Ok(normalized)
}

/// Find the deepest existing ancestor of a path.
/// Returns (existing_ancestor, non_existent_tail).
fn find_existing_ancestor(path: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let mut existing = path.to_path_buf();
    let mut trailing = std::path::PathBuf::new();

    while !existing.exists() {
        if let Some(file_name) = existing.file_name() {
            trailing = std::path::PathBuf::from(file_name).join(&trailing);
        }
        if let Some(parent) = existing.parent() {
            existing = parent.to_path_buf();
        } else {
            // Reached root without finding anything — shouldn't happen since workdir exists
            return (get_workdir().clone(), std::path::PathBuf::from(path));
        }
    }

    (existing, trailing)
}

/// Normalize a path by resolving `..` and `.` components without touching the filesystem.
fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}

// ============================================================================
// Bash
// ============================================================================

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
        if let Err(e) = tx.send(output) {
            eprintln!("Bash tool: failed to send result on channel: {}", e);
        }
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

// ============================================================================
// ReadFile
// ============================================================================

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

// ============================================================================
// WriteFile
// ============================================================================

#[abu_macros::tool(
    struct_name = WriteFile,
    description = "Write content to a file. Creates parent directories automatically. Overwrites existing files."
)]
pub fn run_write(path: &str, content: &str) -> String {
    let fp = match safe_new_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    if let Some(parent) = fp.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("Error creating parent dirs: {}", e);
        }
    }

    match std::fs::write(&fp, content) {
        Ok(_) => format!("Wrote {} bytes to {}", content.len(), path),
        Err(e) => format!("Error when write {}: {}", path, e),
    }
}

// ============================================================================
// EditFile
// ============================================================================

#[abu_macros::tool(
    struct_name = EditFile,
    description = "Edit a file by exact string replacement. Set replace_all=true to replace all occurrences, or leave it false to require exactly one match."
)]
pub fn run_edit(
    path: &str,
    old_string: &str,
    new_string: &str,
    #[arg(
        description = "Replace all occurrences instead of exactly one, default false",
        default = "false"
    )]
    replace_all: bool,
) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    let content = match std::fs::read_to_string(&fp) {
        Ok(c) => c,
        Err(e) => return format!("Error reading file: {}", e),
    };

    if replace_all {
        let count = content.matches(old_string).count();
        if count == 0 {
            return "Error: old_string not found in file".to_string();
        }
        let new_content = content.replace(old_string, new_string);
        match std::fs::write(&fp, &new_content) {
            Ok(_) => format!("Replaced {} occurrences in {}", count, path),
            Err(e) => format!("Error writing file: {}", e),
        }
    } else {
        let count = content.matches(old_string).count();
        if count == 0 {
            return format!("Error: old_string not found in file");
        }
        if count > 1 {
            return format!(
                "Error: old_string matches {} occurrences. Must match exactly one, or set replace_all=true.",
                count
            );
        }
        let new_content = content.replacen(old_string, new_string, 1);
        match std::fs::write(&fp, &new_content) {
            Ok(_) => format!("Successfully edited {}", path),
            Err(e) => format!("Error writing file: {}", e),
        }
    }
}

// ============================================================================
// Glob — file pattern matching
// ============================================================================

#[abu_macros::tool(
    struct_name = Glob,
    description = "Find files matching a glob pattern. Use ** for recursive search. E.g. '**/*.rs' finds all Rust files.",
    category = "safe"
)]
pub fn run_glob(pattern: &str) -> String {
    let workdir = get_workdir();
    let mut results: Vec<String> = vec![];

    // Determine if pattern is recursive
    let (base_dir, file_pattern) = if pattern.contains("**") {
        // Recursive: start from workdir
        (workdir.clone(), pattern.replace("**/", "").replace("**", ""))
    } else if let Some(slash_pos) = pattern.rfind('/') {
        let dir = workdir.join(&pattern[..slash_pos]);
        let pat = pattern[slash_pos + 1..].to_string();
        (dir, pat)
    } else {
        (workdir.clone(), pattern.to_string())
    };

    let is_recursive = pattern.contains("**");

    let walker: Box<dyn Iterator<Item = walkdir::DirEntry>> = if is_recursive {
        Box::new(
            walkdir::WalkDir::new(&base_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file()),
        )
    } else {
        Box::new(
            walkdir::WalkDir::new(&base_dir)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file()),
        )
    };

    for entry in walker {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy();

        // Simple glob matching: * matches anything
        if matches_glob(&file_name, &file_pattern) {
            if let Ok(rel) = path.strip_prefix(workdir) {
                results.push(rel.to_string_lossy().to_string());
            }
        }

        if results.len() >= 200 {
            results.push("... (truncated at 200 results)".to_string());
            break;
        }
    }

    if results.is_empty() {
        format!("No files matching '{}'", pattern)
    } else {
        results.join("\n")
    }
}

fn matches_glob(name: &str, pattern: &str) -> bool {
    if pattern == "*" || pattern.is_empty() {
        return true;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return name == pattern;
    }

    // Pattern like "*.rs" or "foo*.rs" or "*.test.ts"
    let mut remainder = name;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // Must start with this
            if !remainder.starts_with(part) {
                return false;
            }
            remainder = &remainder[part.len()..];
        } else if i == parts.len() - 1 {
            // Must end with this
            if !remainder.ends_with(part) {
                return false;
            }
        } else {
            // Must contain this somewhere
            if let Some(pos) = remainder.find(part) {
                remainder = &remainder[pos + part.len()..];
            } else {
                return false;
            }
        }
    }
    true
}

// ============================================================================
// Grep — content search
// ============================================================================

#[abu_macros::tool(
    struct_name = Grep,
    description = "Search file contents with a regex pattern. Optionally filter by file glob. Returns matching lines with file path and line number.",
    category = "safe"
)]
pub fn run_grep(
    pattern: &str,
    #[arg(
        description = "Optional glob to filter files, e.g. '*.rs' or '**/*.rs'",
        default = "String::new()"
    )]
    path_filter: String,
) -> String {
    let workdir = get_workdir();
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return format!("Invalid regex: {}", e),
    };

    let mut results: Vec<String> = vec![];
    let mut files_scanned = 0u32;
    let mut matches_found = 0u32;

    let walker: Box<dyn Iterator<Item = walkdir::DirEntry>> = {
        if path_filter.contains("**") {
            Box::new(
                walkdir::WalkDir::new(workdir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file()),
            )
        } else if path_filter.is_empty() {
            Box::new(
                walkdir::WalkDir::new(workdir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file()),
            )
        } else if let Some(slash_pos) = path_filter.rfind('/') {
            let dir = workdir.join(&path_filter[..slash_pos]);
            let pat = path_filter[slash_pos + 1..].to_string();
            Box::new(
                walkdir::WalkDir::new(&dir)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(move |e| {
                        e.file_type().is_file()
                            && matches_glob(&e.file_name().to_string_lossy(), &pat)
                    }),
            )
        } else {
            let pat = path_filter.to_string();
            Box::new(
                walkdir::WalkDir::new(workdir)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(move |e| {
                        e.file_type().is_file()
                            && matches_glob(&e.file_name().to_string_lossy(), &pat)
                    }),
            )
        }
    };

    // Skip common binary/dependency directories
    let skip_dirs: [&str; 5] = ["target", "node_modules", ".git", "__pycache__", ".venv"];

    for entry in walker {
        let path = entry.path();

        // Skip binary/dep dirs
        if path
            .components()
            .any(|c| skip_dirs.iter().any(|d| c.as_os_str() == *d))
        {
            continue;
        }

        // Try to read as text
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary files
        };

        files_scanned += 1;

        for (line_no, line) in content.lines().enumerate() {
            if re.is_match(line) {
                let rel_path = path
                    .strip_prefix(workdir)
                    .unwrap_or(path)
                    .to_string_lossy();

                results.push(format!("{}:{}: {}", rel_path, line_no + 1, line));
                matches_found += 1;

                if results.len() >= 100 {
                    results.push(format!(
                        "... (truncated at 100 matches, scanned {} files)",
                        files_scanned
                    ));
                    return results.join("\n");
                }
            }
        }

        if files_scanned >= 1000 {
            results.push(format!(
                "... (stopped after scanning {} files, found {} matches)",
                files_scanned, matches_found
            ));
            return results.join("\n");
        }
    }

    if results.is_empty() {
        format!(
            "No matches for '{}' (scanned {} files)",
            pattern, files_scanned
        )
    } else {
        results.push(format!(
            "-- {} matches in {} files --",
            matches_found, files_scanned
        ));
        results.join("\n")
    }
}
