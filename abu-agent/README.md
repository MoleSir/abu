# abu-agent

Agent development library, featuring a basic Agent Loop, multiple memory strategies, and support for MCP, native Tools, and Skills within an Agent kit.

- Model
- Memory
- Hook
- Middleware
- Mcp
- Function Calling
- Skills


## Examples

A simple task with buildin tools.

```rust
dotenv::from_filename(".env")?;
let llm = DeepSeek::from_env().expect("new deepseek");
let model = std::env::var("MODEL_ID")?;
let mut agent = AgentBuilder::new(llm, model)
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
```

With stdio mcp server

```rust
dotenv::from_filename(".env")?;
let llm = DeepSeek::from_env().expect("new deepseek");
let model = std::env::var("MODEL_ID")?;
let mut agent = AgentBuilder::new(llm, model)
    .with_builtin_tools(true)
    .system_prompt("You are an autonomous task-solving agent.")
    .with_mcpserver("python3", ["./mcp/weather.py"])
    .with_mcpserver("python3", ["./mcp/username.py"])
    .build()
    .await?;

debug!("{:#?}", agent.tool_list());

agent.run("帮我查询上海的天气").await?;
agent.run("我的名字是？").await?;
```



## References

- https://github.com/FareedKhan-dev/optimize-ai-agent-memory.git