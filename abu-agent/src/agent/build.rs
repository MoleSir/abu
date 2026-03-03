use std::path::PathBuf;
use crate::{llm::LLM, memory::{Memory, MemoryStrategy, Sequential}, tool::{bash::Bash, calculate::Calculator, fs::{FileCreator, FileReader, FileWritor}, terminate::Terminator, Tool}, AgentResult};
use super::{Agent, AgentKit};

const DEFAULT_SYSTEM_PROMPT: &str = "";

pub struct AgentBuilder {
    pub llm: LLMBuilder,
    pub memory_strategy: Box<dyn MemoryStrategy>,
    pub system_prompt: Option<String>,
    pub with_skill: Option<PathBuf>,
    pub with_builin_tools: bool,
    pub tools: Vec<Box<dyn Tool>>,
    pub mcpservers: Vec<(String, Vec<String>)>,
}

pub enum LLMBuilder {
    FromEnv,
    With { base_url: String, api_key: String, model: String }
}

impl AgentBuilder {
    pub async fn build(self) -> AgentResult<Agent> {
        let llm = self.llm.build()?;
        
        let memory = Memory::new(self.memory_strategy, self.system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()));
        let mut kit = AgentKit::new();
        kit.add_tool(Terminator::new());

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

        for (cmd, args) in self.mcpservers {
            kit.add_mcp_server(&cmd, &args).await?;
        }

        if let Some(skill_path) = self.with_skill {
            kit.load_skill(skill_path)?;
        }

        Ok(Agent {
            llm,
            memory,
            kit
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

impl Default for AgentBuilder {
    fn default() -> Self {
        Self {
            llm: LLMBuilder::FromEnv,
            memory_strategy: Box::new(Sequential::new()),
            system_prompt: None,
            with_skill: None,
            with_builin_tools: true,
            tools: vec![],
            mcpservers: vec![],
        }
    }
}

impl AgentBuilder {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn with_llm(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            llm: LLMBuilder::With { base_url: base_url.into(), api_key: api_key.into(), model: model.into() },
            ..Default::default()
        }
    }

    pub fn memory_strategy(mut self, memory_strategy: Box<dyn MemoryStrategy>) -> Self {
        self.memory_strategy = memory_strategy;
        self
    }

    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn with_skill(mut self, skill_path: impl Into<PathBuf>) -> Self {
        self.with_skill = Some(skill_path.into());
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
    use crate::agent::AgentBuilder;

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