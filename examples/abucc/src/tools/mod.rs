use tokio::process::Command;
use tokio::time::{timeout, Duration};
use std::path::{Path, PathBuf, Component};
use std::sync::OnceLock;

static WORKDIR: OnceLock<PathBuf> = OnceLock::new();

fn get_workdir() -> &'static PathBuf {
    WORKDIR.get_or_init(|| std::env::current_dir().expect("Failed to get current working directory"))
}

/// 路径安全检查（同步逻辑）
fn safe_path<P: AsRef<Path>>(p: P) -> anyhow::Result<PathBuf> {
    let workdir = get_workdir();
    let mut path = workdir.clone();

    for component in p.as_ref().components() {
        match component {
            Component::ParentDir => { path.pop(); },
            Component::Normal(c) => { path.push(c); }
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
        }
    }

    if !path.starts_with(workdir) {
        anyhow::bail!("Path escapes workspace")
    }
    Ok(path)
}

/// 运行 Shell 命令 (异步版)
#[abu_macros::tool(
    struct_name = Bash,
    description = "Run a shell command in the workspace. Use this to run tests, build projects, or execute scripts.",
)]
pub async fn run_bash(command: &str) -> String {
    run_bash(command).await
}

pub async fn run_bash(command: &str) -> String {
    let dangerous = ["rm -rf /", "sudo", "shutdown", "reboot", "> /dev/"];
    if dangerous.iter().any(|&d| command.contains(d)) {
        return "Error: Dangerous command blocked".to_string();
    }

    let cmd_future = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(get_workdir())
        .output();

    match timeout(Duration::from_secs(120), cmd_future).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            let out = combined.trim();
            
            if out.is_empty() {
                "(no output)".to_string()
            } else {
                out.chars().take(50000).collect()
            }
        }
        Ok(Err(e)) => format!("Error executing command: {}", e),
        Err(_) => "Error: Timeout (120s) exceeded".to_string(),
    }
}

/// 读取文件 (异步版)
#[abu_macros::tool(
    struct_name = ReadFile,
    description = "Read the full content of a file.",
)]
pub async fn run_read(path: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    match tokio::fs::read_to_string(&fp).await {
        Ok(t) => t,
        Err(e) => format!("Error reading file: {}", e),
    }
}

/// 写入文件 (异步版)
#[abu_macros::tool(
    struct_name = WriteFile,
    description = "Write content to a file. Creates parent directories if they don't exist.",
)]
pub async fn run_write(path: &str, content: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    if let Some(parent) = fp.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return format!("Error creating directory: {}", e);
        }
    }

    match tokio::fs::write(&fp, content).await {
        Ok(_) => format!("Successfully wrote to {}", path),
        Err(e) => format!("Error writing file: {}", e),
    }
}

/// 编辑文件：通过行号范围替换内容
/// 这里的 start_line 和 end_line 都是 1-indexed（从1开始计数），且包含首尾。
#[abu_macros::tool(
    struct_name = EditFile,
    description = "Edit a file by replacing a range of lines with new content. \
                  Line numbers are 1-indexed and inclusive. \
                  To insert at a line, use the same start and end line number. \
                  To delete, provide an empty replacement string.",
)]
pub async fn edit_file(
    path: &str,
    start_line: usize,
    end_line: usize,
    replacement: &str,
) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    // 1. 读取原文件内容
    let content = match tokio::fs::read_to_string(&fp).await {
        Ok(c) => c,
        Err(e) => return format!("Error reading file: {}", e),
    };

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let total_lines = lines.len();

    // 2. 边界检查与参数校验
    if start_line == 0 {
        return "Error: start_line must be >= 1".to_string();
    }
    if start_line > end_line {
        return format!("Error: start_line ({}) is greater than end_line ({})", start_line, end_line);
    }
    
    // 如果是空文件，且 Agent 想在第 1 行插入
    if total_lines == 0 && start_line == 1 {
        match tokio::fs::write(&fp, replacement).await {
            Ok(_) => return format!("Successfully initialized file {}", path),
            Err(e) => return format!("Error writing file: {}", e),
        }
    }

    if start_line > total_lines {
        return format!("Error: start_line ({}) exceeds total lines ({})", start_line, total_lines);
    }

    // 3. 执行替换逻辑
    // 转换为 0-indexed
    let start_idx = start_line - 1;
    // 如果 end_line 超过总行数，则截断到末尾
    let end_idx = end_line.min(total_lines);

    // replacement 可能是多行，需要按行拆分
    let new_lines: Vec<String> = replacement.lines().map(|s| s.to_string()).collect();
    
    // 使用 Vec 的 splice 方法替换范围
    lines.splice(start_idx..end_idx, new_lines);

    // 4. 写回文件
    let new_content = lines.join("\n");
    match tokio::fs::write(&fp, new_content).await {
        Ok(_) => {
            let change = (end_idx - start_idx) as i32 - (replacement.lines().count() as i32);
            format!(
                "Successfully edited lines {}-{} in {}. Net line change: {}", 
                start_line, end_line, path, if change > 0 { format!("-{}", change) } else { format!("+{}", change.abs()) }
            )
        },
        Err(e) => format!("Error writing file: {}", e),
    }
}

#[abu_macros::tool(
    struct_name = ReplaceText,
    description = "Find a specific string (pattern) and replace it with another string (replacement).",
)]
pub async fn replace_text(path: &str, pattern: &str, replacement: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    let content = match tokio::fs::read_to_string(&fp).await {
        Ok(c) => c,
        Err(e) => return format!("Error reading file: {}", e),
    };

    if !content.contains(pattern) {
        return "Error: Pattern not found in file.".to_string();
    }

    // 检查模式是否唯一，如果不唯一，Agent 可能会改错地方
    if content.matches(pattern).count() > 1 {
        return "Error: Pattern is ambiguous (found multiple matches). Provide more context.".to_string();
    }

    let new_content = content.replace(pattern, replacement);
    match tokio::fs::write(&fp, new_content).await {
        Ok(_) => "Successfully replaced text.".to_string(),
        Err(e) => format!("Error: {}", e),
    }
}

#[abu_macros::tool(
    struct_name = ListFiles,
    description = "List files in a directory. Use recursive=true for deep search.",
)]
pub async fn list_files(path: &str, recursive: bool) -> String {
    let root = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    let mut result = Vec::new();
    let mut stack = vec![root];

    while let Some(current_dir) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let p = entry.path();
            let relative = p.strip_prefix(get_workdir()).unwrap_or(&p).to_string_lossy();
            
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                result.push(format!("{}/", relative));
                if recursive {
                    stack.push(p);
                }
            } else {
                result.push(relative.to_string());
            }
            
            if result.len() > 1000 { 
                return format!("{}\n... (too many files, truncated)", result.join("\n"));
            }
        }
    }
    result.join("\n")
}

#[abu_macros::tool(
    struct_name = SearchCode,
    description = "Search for a pattern in all files within the workspace (similar to grep).",
)]
pub async fn search_code(pattern: &str) -> String {
    let cmd = format!("grep -rnE {:?} . --exclude-dir=.git", pattern);
    run_bash(&cmd).await
}

#[abu_macros::tool(
    struct_name = FileInfo,
    description = "Get file metadata (size, modified time).",
)]
pub async fn file_info(path: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    match tokio::fs::metadata(&fp).await {
        Ok(m) => format!(
            "Size: {} bytes\nModified: {:?}\nIs Directory: {}", 
            m.len(), m.modified().ok().unwrap_or(std::time::SystemTime::UNIX_EPOCH), m.is_dir()
        ),
        Err(e) => format!("Error: {}", e),
    }
}

#[abu_macros::tool(
    struct_name = ReadFileRange,
    description = "Read specific line range of a file (1-indexed).",
)]
pub async fn read_file_range(path: &str, start_line: usize, end_line: usize) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    let content = match tokio::fs::read_to_string(&fp).await {
        Ok(t) => t,
        Err(e) => return format!("Error: {}", e),
    };

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start = start_line.saturating_sub(1).min(total);
    let end = end_line.min(total);

    if start >= end {
        return "Error: Invalid line range".to_string();
    }

    lines[start..end].join("\n")
}