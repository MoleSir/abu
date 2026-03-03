use abu_agent::agent::AgentBuilder;
use tracing::{debug, info, level_filters::LevelFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(LevelFilter::INFO)
        .with_level(true)
        .init();

    info!("start main");
    if let Err(e) = result_main().await {
        eprint!("Err: {}", e);
    }
}

async fn result_main() -> Result<(), Box<dyn std::error::Error>> {
    let mut agent = AgentBuilder::from_env()
        .with_builin_tools(true)
        .system_prompt("You are a asistant")
        .with_mcpserver("python3", ["./mcp/weather.py"])
        .with_mcpserver("python3", ["./mcp/username.py"])
        .build()
        .await?;

    debug!("{:#?}", agent.tool_list());
    
    // agent.run("帮我查询上海的天气").await?;
    agent.run("我的名字是？").await?;

    Ok(())
} 