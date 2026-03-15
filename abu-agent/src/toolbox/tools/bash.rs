use std::process::Command;

#[abu_macros::tool(
    struct_name = Bash,
    description = "Run a shell command.",
)]
pub fn bash(command: &str) -> String {
    match Command::new("sh")
        .arg("-c")
        .arg(command)
        .output() {
        Ok(output) => {
            if output.status.success() {
                format!("Execute command with stdout: {}", String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                format!("Execute command with stderr: {}", String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        Err(err) => {
            format!("Failed to execute command because of {}", err.to_string())
        }
    }
}