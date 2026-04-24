use std::io::Write;
use abu_agent::{hook::ConsoleLoggerHook, model::ChatModel, AgentBuilder};
use std::process::Command;

#[tokio::main]
async fn main() {
    // tracing_subscriber::fmt()
    //     .with_target(false)
    //     .with_level(true)
    //     .init();

    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

#[abu_macros::tool(
    struct_name = Bash,
    description = "Run a shell command.",
)]
pub fn bash(command: &str) -> String {
    let dangerous = ["rm -rf /", "sudo", "shutdown", "reboot", "> /dev/"];
    if dangerous.iter().any(|item| command.contains(item)) {
        return "Error: Dangerous command blocked".to_string()
    }

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

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?.join("workspace");
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use bash to inspect and change the workspace. Act first, then report clearly.", cur_path))
        .with_builtin_tools(false)
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .build().await?;

    loop {
        print!("s01 >> ");
        std::io::stdout().flush()?;
        
        let mut query = String::new();
        std::io::stdin().read_line(&mut query)?;
        let query = query.trim();
        if query == "q" || query == "quit" || query.is_empty() {
            break;
        }
        
        agent.run(query).await?;
    }

    Ok(())
}