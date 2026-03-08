use std::{path::PathBuf, sync::Arc};
use abu_tool::Tool;
use tokio::sync::RwLock;
use crate::{context::ContextBuilder, kit::tools::{bash::Bash, calculate::Calculator, fs::{FileCreator, FileReader, FileWritor}, terminate::Terminator}, llm::LLM, memory::{Memory, SequentialMemory}, AgentResult};
use super::{Agent, AgentConfig, AgentKit};

const DEFAULT_SYSTEM_PROMPT: &str = "You are an agent.";

pub struct AgentBuilder<M: Memory = SequentialMemory> {
    pub llm: LLMBuilder,
    pub config: AgentConfig,
    pub memory: M,
    pub system_prompt: String,
    pub with_skills: Option<PathBuf>,
    pub with_builin_tools: bool,
    pub with_subagent: bool,
    pub tools: Vec<Box<dyn Tool>>,
    pub mcpservers: Vec<(String, Vec<String>)>,
    pub mcpconfig_path: Option<PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iteration: 10,
            temperature: 0.7,
        }
    }
}

pub enum LLMBuilder {
    FromEnv,
    With { base_url: String, api_key: String, model: String }
}

impl<M: Memory> AgentBuilder<M> {
    pub async fn build(self) -> AgentResult<Agent<M>> {
        let llm = self.llm.build()?;
        let mut system_prompt = format!("{}\nOnce you consider the work complete or do task to do, call the terminate method.", self.system_prompt);
        let mut kit = AgentKit::new();
        kit.add_tool(Terminator::new());

        // tool
        if self.with_builin_tools {
            kit.add_tool(Bash::new());
            kit.add_tool(Calculator::new());
            kit.add_tool(FileCreator::new());
            kit.add_tool(FileWritor::new());
            kit.add_tool(FileReader::new());
        }

        for tool in self.tools {
            kit.add_tool_box(tool);
        }

        // mcp
        if let Some(path) = self.mcpconfig_path {
            kit.load_mcpconfig(&path).await?;
        }

        for (cmd, args) in self.mcpservers {
            kit.add_mcp_server(&cmd, &args).await?;
        }

        // skill
        if let Some(skill_path) = self.with_skills {
            kit.load_skill(skill_path)?;
            system_prompt = kit.attach_system_prompt(&system_prompt);   
        }

        // context builder
        let context_builder = ContextBuilder::new(system_prompt);

        Ok(Agent {
            config: self.config,
            llm: Arc::new(llm),
            memory: self.memory,
            kit: Arc::new(RwLock::new(kit)),
            context_builder
        })

    }
}

impl LLMBuilder {
    pub fn build(self) -> AgentResult<LLM> {
        match self {
            Self::FromEnv => LLM::from_env(),
            Self::With { base_url, api_key, model } => Ok(LLM::new(base_url, api_key, model))
        }
    }
}

impl<M: Memory + Default> Default for AgentBuilder<M> {
    fn default() -> Self {
        Self {
            llm: LLMBuilder::FromEnv,
            config: AgentConfig::default(),
            memory: M::default(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            with_skills: None,
            with_builin_tools: true,
            with_subagent: false,
            tools: vec![],
            mcpservers: vec![],
            mcpconfig_path: None,
        }
    }
}

impl AgentBuilder<SequentialMemory> {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn with_llm(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            llm: LLMBuilder::With { base_url: base_url.into(), api_key: api_key.into(), model: model.into() },
            ..Default::default()
        }
    }
}

impl<M: Memory> AgentBuilder<M> {
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.config.temperature = temperature;
        self
    }

    pub fn max_iteration(mut self, max_iteration: usize) -> Self {
        self.config.max_iteration = max_iteration;
        self
    }

    pub fn memory<NM: Memory>(self, memory: NM) -> AgentBuilder<NM> {
        AgentBuilder {
            memory,
            llm: self.llm,
            config: self.config,
            system_prompt: self.system_prompt,
            with_skills: self.with_skills,
            with_builin_tools: self.with_builin_tools,
            with_subagent: self.with_subagent,
            tools: self.tools,
            mcpservers: self.mcpservers,
            mcpconfig_path: self.mcpconfig_path
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

    pub fn with_builin_tools(mut self, enabled: bool) -> Self {
        self.with_builin_tools = enabled;
        self
    }

    pub fn with_tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
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

    pub fn with_mcpserver<'a>(mut self, cmd: &str, args: impl IntoIterator<Item = &'a str>) -> Self {
        let args = args.into_iter().collect::<Vec<_>>();
        let cmd = cmd.to_string();
        let args = args.into_iter()
            .map(|arg| arg.to_string())
            .collect();
        self.mcpservers.push((cmd, args));
        self
    }
}

#[cfg(test)]
mod test {
    use super::AgentBuilder;

    #[tokio::test]
    async fn test_build() {
        AgentBuilder::from_env()
            .system_prompt("hihi")
            .with_builin_tools(true)
            .build()
            .await
            .expect("build llm");
        
    }
    
}