use std::io::Write;

use abu_agent::{model::ChatModel, tool::Bash, AgentBuilder};
use abucc::tools::{ReadFile, WriteFile};


#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use bash to inspect and change the workspace. Act first, then report clearly.", cur_path))
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .build().await?;

    loop {
        print!("abucc >> ");
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
