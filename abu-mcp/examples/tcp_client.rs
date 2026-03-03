use tokio::net::TcpStream;
use abu_mcp::{
    client::McpClient, transport::tcp::McpTcpTransport, 
    McpToolCall
};
use tokio;
use tracing::info;

async fn result_main() -> anyhow::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:020716").await?;
    let transport = McpTcpTransport::new(stream).await;

    let mut client = McpClient::new(transport);
 
    info!("Initializing MCP client...");
    let server_capabilities = client.initialize().await?;
    info!("Server capabilities: {:?}", server_capabilities);

    info!("Sending test tools/list...");
    let tools = client.tools_list().await?;
    for tool in tools {
        info!("{:?}", tool);
    }

    info!("Call a tool!");
    let tool_call = McpToolCall {
        name: "get_my_name".to_string(),
        arguments: None,   
    };
    let result = client.tools_call(tool_call).await?;
    info!("Get result: '{:?}'", result.content[0]);

    info!("Shutting down...");
    client.shutdown().await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();
    if let Err(err) = result_main().await {
        eprintln!("{}", err);
    }
}