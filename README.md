# Abu

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

Abu is a llm development library, created purely for personal learning.



## Structure

- [abu-base](./abu-base/): Core data structures for chat and embeddings.
- [abu-provider](./abu-provider/): API integration supporting multiple vendors.
- [abu-tool](./abu-tool/): Agent tool abstractions, with support for quickly generating Tool objects using tools from abu-macros
- [abu-mcp](./abu-mcp/): MCP protocol implementation, including an optional fastmcp module, designed similarly to Python’s fastmcp API.
- [abu-skill](./abu-skill/): Skill loading and management.
- [abu-rag](./abu-rag/): Document loading, splitting, embedding, and simple vector database operations.
- [abu-agent](./abu-agent/): Agent development library, featuring a basic Agent Loop, multiple memory strategies, and support for MCP, native Tools, and Skills within an Agent kit.



## LICENSE

MIT