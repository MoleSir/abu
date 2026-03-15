use abu_agent::{model::ChatModel, AgentBuilder};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("start main");
    if let Err(e) = result_main().await {
        eprint!("Err: {}", e);
    }
}

async fn result_main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::from_filename(".env")?;
    let llm = ChatModel::deepseek(std::env::var("MODEL_ID")?)?;
    let mut agent = AgentBuilder::new(llm)
        .with_builtin_tools(true)
        .system_prompt(
r#"You are a senior software engineer.
You write clean, idiomatic, production-ready code.
Prefer correctness, clarity, and robustness over verbosity.
Avoid unnecessary explanation unless explicitly requested."#
        )
        .build()
        .await?;

    agent.run("编写一个冒泡排序代码，写入到 ./temp/sort.py 中").await?;

    Ok(())
} 