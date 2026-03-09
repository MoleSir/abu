use std::path::PathBuf;
use abu_provider::{deepseek::DeepSeek, ChatProvide};
use abu_tool::Tool;
use crate::{context::ContextBuilder, kit::tools::{bash::Bash, calculate::Calculator, fs::{FileCreator, FileReader, FileWritor}}, memory::{Memory, SequentialMemory}, AgentResult};
use super::{Agent, AgentConfig, AgentKit};

const DEFAULT_SYSTEM_PROMPT: &str = "You are an agent.";

pub struct AgentBuilder<C: ChatProvide = DeepSeek, M: Memory = SequentialMemory> {
    pub llm: C,
    pub model: String,
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

impl<C: ChatProvide, M: Memory> AgentBuilder<C, M> {
    pub async fn build(mut self) -> AgentResult<Agent<C, M>> {
        // let mut system_prompt = format!("{}\nOnce you consider the work complete or do task to do, call the terminate method.", self.system_prompt);
        let mut kit = AgentKit::new();
        // kit.add_tool(Terminator::new());

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
            self.system_prompt = kit.attach_system_prompt(&self.system_prompt);   
        }

        // context builder
        let context_builder = ContextBuilder::new(self.system_prompt);

        Ok(Agent {
            config: self.config,
            llm: self.llm,
            model: self.model,
            memory: self.memory,
            kit,
            context_builder
        })

    }
}

impl<C: ChatProvide> AgentBuilder<C> {
    pub fn new(llm: C, model: impl Into<String>) -> Self {
        Self {
            llm,
            model: model.into(),
            config: AgentConfig::default(),
            memory: SequentialMemory::default(),
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
            model: self.model,
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

    pub fn llm<NC: ChatProvide>(self, llm: NC) -> AgentBuilder<NC, M> {
        AgentBuilder {
            memory: self.memory,
            llm,
            model: self.model,
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

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
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
    use abu_provider::deepseek::DeepSeek;

    use super::AgentBuilder;

    #[tokio::test]
    async fn test_build() {
        dotenv::from_filename(".env").unwrap();
        let deepseek = DeepSeek::from_env().expect("new deepseek");
        let model = std::env::var("MODEL_ID").unwrap();
        AgentBuilder::new(deepseek, model)
            .system_prompt("hihi")
            .with_builin_tools(true)
            .build()
            .await
            .expect("build llm");
        
    }
    
}