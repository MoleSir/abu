# Abu Code

基于 [abu-agent](https://github.com/MoleSir/abu) 框架构建的编码 Agent CLI。可以理解为轻量级、可定制的 Claude Code 替代品。

## 特性

- **对话式编程** — 自然语言驱动代码修改，保留完整上下文
- **18 个内置工具** — Bash、Read、Write、Edit、Glob、Grep 等
- **任务系统** — 强制任务跟踪，支持依赖链（blocked_by）
- **持久化记忆** — 跨会话记住偏好、反馈和项目知识
- **会话恢复** — 对话历史保存为 JSONL，重启自动恢复
- **3 种子 Agent** — `task`（读写执行）、`explore`（只读搜索）、`plan`（架构设计）
- **后台任务** — 长时间命令异步执行，完成自动通知
- **权限系统** — 3 种模式（Auto / Plan / Default），危险命令拦截
- **增量上下文压缩** — 只压缩新消息，基于字符数阈值，避免重复压缩
- **技能系统** — 从 `./skills/` 目录加载领域知识
- **MCP 支持** — 通过 `.mcp.json` 接入外部工具服务器
- **CLAUDE.md** — 加载用户全局（`~/.claude/CLAUDE.md`）和项目级指令

## 安装

```bash
git clone https://github.com/MoleSir/abu.git
cd abu/examples/abu-code
cargo build --release
```

## 快速开始

创建 `.env` 文件：

```env
CHAT_MODEL=deepseek-chat
DEEPSEEK_API_KEY=sk-your-key-here
```

运行：

```bash
cargo run
```

```
Abu Code  |  Model: deepseek-chat
Project:   "/home/you/project"
Data dir:  "~/.abu-code/projects/home-you-project-a1b2c3d4"
Mode:      Auto
Type /help for commands.
>
```

## 配置

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `CHAT_MODEL` | `deepseek-chat` | 模型名称 |
| `CHAT_MODEL_COMPACT` | 同 `CHAT_MODEL` | 上下文压缩用模型 |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | API 端点（兼容所有 OpenAI 格式 API） |
| `DEEPSEEK_API_KEY` | — | API 密钥 |

使用 OpenAI：

```env
CHAT_MODEL=gpt-4o
DEEPSEEK_BASE_URL=https://api.openai.com/v1
DEEPSEEK_API_KEY=sk-openai-key
```

使用本地模型：

```env
CHAT_MODEL=llama-3-70b
DEEPSEEK_BASE_URL=http://localhost:8080/v1
DEEPSEEK_API_KEY=not-needed
```

## 命令

| 命令 | 说明 |
|------|------|
| `/help` | 显示所有命令 |
| `/tools` | 列出所有已注册工具 |
| `/mode` | 查看当前权限模式 |
| `/plan` | 切换到 Plan 模式（只读） |
| `/auto` | 切换到 Auto 模式（安全工具自动批准） |
| `/default` | 切换到 Default 模式（全部询问） |
| `/memory` | 列出已保存的记忆 |
| `/tasks` | 列出当前任务 |
| `/sessions` | 列出已保存的会话 |
| `/clear` | 开始新会话 |
| `/save` | 手动保存当前会话 |
| `/quit` | 退出（自动保存） |

## 工具

### 文件操作
- **Bash** — 执行 Shell 命令（120 秒超时，危险命令拦截）
- **ReadFile** — 读取文件内容
- **WriteFile** — 写入文件（自动创建父目录）
- **EditFile** — 精确字符串替换，支持 `replace_all` 批量替换

### 代码探索
- **Glob** — 文件模式匹配（`**/*.rs` 递归搜索）
- **Grep** — 正则搜索文件内容，支持文件过滤

### 任务管理
- **task_create** — 创建任务，可指定 `blocked_by` 依赖
- **task_update** — 更新状态、添加依赖关系
- **task_list** — 列出任务及阻塞状态
- **task_get** — 查看任务详情

### 记忆
- **save_memory** — 持久化记忆（user/feedback/project/reference 四种类型）

### 后台任务
- **background_run** — 异步执行长命令
- **background_check** — 查询任务状态和输出
- **background_list** — 列出所有后台任务

### 子 Agent
- **task** — 通用子 Agent（读、写、编辑、执行）
- **explore** — 只读探索子 Agent（Glob、Grep、Read）
- **plan** — 架构设计子 Agent（只读，输出分步实施计划）

## 数据存储

所有数据存储在 `~/.abu-code/projects/<路径-slug>-<哈希>/`：

```
~/.abu-code/projects/home-you-project-a1b2c3d4/
├── memory/           # 持久化记忆（Markdown + frontmatter）
├── tasks/            # 任务文件（JSON）
├── sessions/         # 对话历史（JSONL）
├── background/       # 后台任务日志
└── tool_results/     # 大型工具输出缓存
```

## 项目结构

```
abu-code/
├── Cargo.toml
└── src/
    ├── main.rs          # 入口、REPL 循环、Agent 组装
    ├── config.rs        # 从环境变量读取模型配置
    ├── system_prompt.rs # 动态系统提示 + CLAUDE.md 加载
    ├── tools.rs         # Bash、Read、Write、Edit、Glob、Grep
    ├── task.rs          # 任务系统（依赖关系 + 中间件注入）
    ├── memory.rs        # 持久化记忆系统
    ├── session.rs       # 会话 JSONL 保存/加载
    ├── background.rs    # 异步后台任务执行
    ├── compact.rs       # 增量上下文摘要压缩
    ├── permission.rs    # 权限管理器 + 用户授权
    ├── hook.rs          # 终端输出格式化（类似 Claude Code 风格）
    └── subagent.rs      # Task、Explore、Plan 子 Agent 工厂
```

## 许可证

MIT — 详见根目录 [LICENSE](../../LICENSE) 文件。
