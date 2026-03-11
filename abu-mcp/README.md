# abu-mcp


## Features

- Server/Client
  - [x] initialize
  - [x] shutdown
  - [x] tools/list
  - [x] tools/call
- Transport
  - [x] Stdio
  - [x] Tcp
- Fastmcp


## Examples

Based on Python’s MCP, fastmcp is implemented, and the usage is very similar! You can create a Tool simply by decorating a function with a tool macro!


```rust
use abu_mcp::{fastmcp::prelude::*, transport::stdio::McpStdioTransport};

#[tool(
    struct_name = GetMyName,
    description = "Return my name",
)]
fn get_my_name() -> String {
    "molesir".into()
}

#[tool(
    struct_name = Say,
    description = "print something to stdout!",
)]
fn say(#[arg(description = "the string need to print!")] value: &str) {
    println!("{}", value)
}

#[tool(
    struct_name = GetWeather,
    description = "get weather in given place",
)]
fn get_weather(#[arg(description = "the place you want to know weather")] place: &str) -> Result<String> {
    match place {
        "Shanghai" | "上海" => Ok("Sunny".into()),
        _ => Err(anyhow::anyhow!("Unspport place!")),
    }
}

#[tokio::main]
async fn main() {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(GetMyName::new()), Box::new(Say::new()), Box::new(GetWeather::new())
    ];
    let mcp = FastMcp::new(McpStdioTransport::new(), tools);
    mcp.run().await.unwrap()
}
```



## References

- https://github.com/conikeec/mcpr.git
- https://github.com/MarkTechStation/VideoCode
- https://github.com/Derek-X-Wang/mcp-rust-sdk.git 
- https://modelcontextprotocol.io/introduction
- https://modelcontextprotocol.io/specification/2024-11-05