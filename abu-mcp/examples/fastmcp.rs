use abu_mcp::{fastmcp::prelude::*, transport::stdio::McpStdioTransport};

#[mcp_tool(
    struct_name = GetMyName,
    name = "get_my_name",
    description = "Return my name",
    args = []
)]
fn get_my_name() -> String {
    "molesir".into()
}

#[mcp_tool(
    struct_name = Say,
    name = "print something to stdout!",
    description = "Return my thing",
    args = [
        "value": {
            "title": "Value",
            "description": "the string need to print!",
            "type": "string"
        }
    ]
)]
fn say(value: &str) {
    println!("{}", value);
}

#[mcp_tool(
    struct_name = GetWeather,
    name = "get_weather",
    description = "get weather in given place",
    args = [
        "place": {
            "title": "place",
            "description": "the place you want to know weather",
            "type": "string"
        }
    ]
)]
fn get_weather(place: &str) -> Result<String> {
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

