use std::{path::PathBuf, sync::Arc};
use abu_provider::{deepseek::DeepSeek, ChatProvide};
use abu_skill::SkillLoader;
use abu_tool::Tool;
use crate::{
    context::ContextBuilder, hook::{Hook, HookManager}, memory::{Memory, SequentialMemory}, middleware::{LlmInputMiddleware, LlmOutMiddleware, MemoryAddMiddleware, Middleware, MiddlewareManager, ToolCallMiddleware, ToolResultMiddleware}, model::{ChatConfig, ChatModel}, toolbox::{tools::{bash::Bash, calculate::Calculator, fs::{FileCreator, FileReader, FileWriter}, skill::SkillTool}, PermissionManager, SubAgentTool}, AgentResult
};
use super::{Agent, AgentConfig, ToolBox};

const DEFAULT_SYSTEM_PROMPT: &str = "You are an agent.";

pub struct AgentBuilder<P: ChatProvide = DeepSeek, M: Memory = SequentialMemory> {
    pub llm: ChatModel<P>,
    pub config: AgentConfig,
    pub memory: M,
    pub system_prompt: String,
    pub with_skills: Option<PathBuf>,
    pub with_builtin_tools: bool,
    pub with_subagent: bool,
    pub tools: Vec<Box<dyn Tool>>,
    pub mcpservers: Vec<(String, Vec<String>)>,
    pub mcpconfig_path: Option<PathBuf>,
    pub hooks: HookManager,
    pub middlewares: MiddlewareManager,
    pub permission_manager: Option<PermissionManager>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iteration: 10,
            temperature: 0.7,
        }
    }
}

impl<C: ChatProvide, M: Memory> AgentBuilder<C, M> {
    pub async fn build(mut self) -> AgentResult<Agent<C, M>> {
        let mut toolbox = ToolBox::new();
        let mut context_builder = ContextBuilder::new(self.system_prompt);

        // tool
        if self.with_builtin_tools {
            toolbox.add_tool(Bash::new());
            toolbox.add_tool(Calculator::new());
            toolbox.add_tool(FileCreator::new());
            toolbox.add_tool(FileWriter::new());
            toolbox.add_tool(FileReader::new());
        }
        for tool in self.tools {
            toolbox.add_tool_box(tool);
        }

        // mcp
        if let Some(path) = self.mcpconfig_path {
            toolbox.load_mcpconfig(&path).await?;
        }
        for (cmd, args) in self.mcpservers {
            toolbox.add_mcp_server(&cmd, &args).await?;
        }

        // skill
        if let Some(skill_dir) = self.with_skills {
            let skill_loader = Arc::new(SkillLoader::load(skill_dir)?);
            context_builder.with_skill(skill_loader.clone());
            toolbox.add_tool(SkillTool::new(skill_loader));
        }

        // llm init
        self.llm.bind_tool_defines(toolbox.tool_definitions()).await;
        self.llm.set_config(ChatConfig { temperature: Some(self.config.temperature) });

        // permission
        if let Some(permission_manager) = self.permission_manager {
            toolbox.set_permission(permission_manager);
        }        

        Ok(Agent {
            config: self.config,
            llm: self.llm,
            memory: self.memory,
            toolbox,
            context_builder,
            hooks: self.hooks,
            middlewares: self.middlewares,
        })

    }
}

impl<P: ChatProvide> AgentBuilder<P> {
    pub fn new(llm: ChatModel<P>) -> Self {
        Self {
            llm,
            config: AgentConfig::default(),
            memory: SequentialMemory::default(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            with_skills: None,
            with_builtin_tools: false,
            with_subagent: false,
            tools: vec![],
            mcpservers: vec![],
            mcpconfig_path: None,
            hooks: HookManager::new(),
            middlewares: MiddlewareManager::new(),
            permission_manager: None,
        }
    }
}

impl<C: ChatProvide, M: Memory> AgentBuilder<C, M> {
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.config.temperature = temperature;
        self
    }

    pub fn max_iteration(mut self, max_iteration: usize) -> Self {
        self.config.max_iteration = max_iteration;
        self
    }

    pub fn memory<NM: Memory>(self, memory: NM) -> AgentBuilder<C, NM> {
        AgentBuilder {
            memory,
            llm: self.llm,
            config: self.config,
            system_prompt: self.system_prompt,
            with_skills: self.with_skills,
            with_builtin_tools: self.with_builtin_tools,
            with_subagent: self.with_subagent,
            tools: self.tools,
            mcpservers: self.mcpservers,
            mcpconfig_path: self.mcpconfig_path,
            hooks: self.hooks,
            middlewares: self.middlewares,
            permission_manager: self.permission_manager
        }
    }

    pub fn llm<NC: ChatProvide>(self, llm: ChatModel<NC>) -> AgentBuilder<NC, M> {
        AgentBuilder {
            memory: self.memory,
            llm,
            config: self.config,
            system_prompt: self.system_prompt,
            with_skills: self.with_skills,
            with_builtin_tools: self.with_builtin_tools,
            with_subagent: self.with_subagent,
            tools: self.tools,
            mcpservers: self.mcpservers,
            mcpconfig_path: self.mcpconfig_path,
            hooks: self.hooks,
            middlewares: self.middlewares,
            permission_manager: self.permission_manager
        }
    }

    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = system_prompt.into();
        self
    }

    pub fn with_skills(mut self, skill_path: impl Into<PathBuf>) -> Self {
        self.with_skills = Some(skill_path.into());
        self
    }

    pub fn with_builtin_tools(mut self, enabled: bool) -> Self {
        self.with_builtin_tools = enabled;
        self
    }

    pub fn with_tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    #[inline]
    pub fn with_subagent<AP: ChatProvide + 'static, AM: Memory + 'static>(self, agent: SubAgentTool<AP, AM>) -> Self {
        self.with_tool(agent)
    }

    pub fn with_hook(mut self, hook: impl Hook + 'static) -> Self {
        self.hooks.add_hook(hook);
        self
    }

    pub fn with_middleware(mut self, middleware: impl Into<Middleware>) -> Self {
        self.middlewares.add_middleware(middleware);
        self
    }

    pub fn with_llm_input_middleware<LM: LlmInputMiddleware + 'static>(mut self, middleware: LM) -> Self {
        self.middlewares.add_llm_input(middleware);
        self
    }

    pub fn with_llm_out_middleware<LM: LlmOutMiddleware + 'static>(mut self, middleware: LM) -> Self {
        self.middlewares.add_llm_out(middleware);
        self
    }

    pub fn with_tool_call_middleware<TM: ToolCallMiddleware + 'static>(mut self, middleware: TM) -> Self {
        self.middlewares.add_tool_call(middleware);
        self
    }

    pub fn with_tool_result_middleware<TM: ToolResultMiddleware + 'static>(mut self, middleware: TM) -> Self {
        self.middlewares.add_tool_result(middleware);
        self
    }

    pub fn with_memory_add_middleware<MM: MemoryAddMiddleware + 'static>(mut self, middleware: MM) -> Self {
        self.middlewares.add_memory_add(middleware);
        self
    }

    pub fn with_tools(mut self, tools: impl IntoIterator<Item = Box<dyn Tool>>) -> Self {
        for tool in tools.into_iter() {
            self.tools.push(tool);
        }
        self
    }

    pub fn with_mcpconfig(mut self, path: impl Into<PathBuf>) -> Self {
        self.mcpconfig_path = Some(path.into());
        self
    }

    pub fn with_mcpserver<S1: Into<String>, S2: Into<String>, I: IntoIterator<Item = S2>>(mut self, cmd: S1, args: I) -> Self {
        let args = args.into_iter().collect::<Vec<_>>();
        let cmd = cmd.into();
        let args = args.into_iter()
            .map(|arg| arg.into())
            .collect();
        self.mcpservers.push((cmd, args));
        self
    }

    pub fn with_permission(mut self, permission_manager: PermissionManager) -> Self {
        self.permission_manager = Some(permission_manager);
        self
    }
}

#[cfg(test)]
mod test {
    use crate::model::ChatModel;
    use super::AgentBuilder;

    #[tokio::test]
    async fn test_build() {
        dotenv::from_filename(".env").unwrap();
        let model = ChatModel::deepseek("deepseek-chat").unwrap();
        AgentBuilder::new(model)
            .system_prompt("hihi")
            .with_builtin_tools(true)
            .build()
            .await
            .expect("build llm");
        
    }
    
}