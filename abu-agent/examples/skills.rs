use abu_agent::AgentBuilder;
use abu_provider::deepseek::DeepSeek;
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
    dotenv::from_filename(".env")?;
    let llm = DeepSeek::from_env().expect("new deepseek");
    let model = std::env::var("MODEL_ID")?;
    let mut agent = AgentBuilder::new(llm, model)
        .with_builin_tools(true)
        .system_prompt("You are an agent.")
        .with_skills("./skills")
        .build()
        .await?;

    info!("{}", agent.system_prompt());
    debug!("{:#?}", agent.tool_list());
    
    agent.run("What skills are available? just tell me all skill's name").await?;
    agent.run("Load the agent-builder skill and follow its instructions").await?;
    agent.run("I need to do a code review -- load the relevant skill first").await?;
    agent.run("Build an MCP server using the mcp-builder skill, save code to ./temp/mcp.py").await?;

    Ok(())
} 