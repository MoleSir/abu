use abu_base::chat::ChatMessage;
use abu_provider::{deepseek::DeepSeek, openai::OpenAi};
use tracing::{debug, info, level_filters::LevelFilter};
use crate::{memory::{RetrievalMemory, SliceWindowMemory, SummarizationMemory}, model::ChatModel};
use super::{Memory, SequentialMemory};

fn load_chat() -> ChatModel<DeepSeek> {
    dotenv::dotenv().unwrap();
    let model = std::env::var("CHAT_MODEL").unwrap();
    let deepseek = ChatModel::deepseek(model).unwrap();
    deepseek
}

fn load_embed() -> (OpenAi, String) {
    dotenv::dotenv().unwrap();
    let openai = OpenAi::from_env().expect("oi");
    let model = std::env::var("EMBED_MODEL").unwrap();
    (openai, model)
}

struct AiAgent<M> {
    pub memory: M,
    pub deepseek: ChatModel<DeepSeek>,
    pub system_prompt: String,
}

impl<M: Memory> AiAgent<M> {
    fn new(memory: M, system_prompt: impl Into<String>) -> Self {
        dotenv::dotenv().unwrap();
        let deepseek = load_chat();
        Self { memory, deepseek, system_prompt: system_prompt.into() }
    }

    async fn chat(&mut self, user_input: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("=======================================================================");
        info!("User > {user_input}");
        let context = self.memory.search(user_input).await?;
        for c in context.iter() {
            debug!("Context: {} {}", c.role(), c.content());
        }
        
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];
        messages.extend(context);
        messages.push(ChatMessage::user(user_input));

        let ai_response = self.deepseek.chat(messages).await?.message.content;

        info!("Agent > {ai_response}");
        self.memory.add(user_input, &ai_response).await?;

        Ok(())
    }
}

fn init_tracing(filter: LevelFilter) {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(filter)
        .with_level(true)
        .init();
}

#[tokio::test]
async fn test_sequential() {
    init_tracing(LevelFilter::INFO);

    let memory = SequentialMemory::new();
    let mut agent = AiAgent::new(memory, "You are a helpful AI assistant.");

    agent.chat("Hi there! My name is MoleSir.").await.unwrap();
    agent.chat("Who are you?").await.unwrap();
    agent.chat("What was the first thing I told you?").await.unwrap();
}

#[tokio::test]
async fn test_sliding_window_1() {
    init_tracing(LevelFilter::INFO);

    let memory = SliceWindowMemory::new(2);
    let mut agent = AiAgent::new(memory, "You are a helpful AI assistant.");

    agent.chat("Hi there! My name is MoleSir.").await.unwrap();
    agent.chat("Who are you?").await.unwrap();
    // forget
    agent.chat("Do you know my name?").await.unwrap();
}

#[tokio::test]
async fn test_sliding_window_2() {
    init_tracing(LevelFilter::INFO);

    let memory = SliceWindowMemory::new(3);
    let mut agent = AiAgent::new(memory, "You are a helpful AI assistant.");

    agent.chat("Hi there! My name is MoleSir.").await.unwrap();
    agent.chat("Who are you?").await.unwrap();
    agent.chat("Do you know my name?").await.unwrap(); // MoleSir
}

#[tokio::test]
async fn test_sliding_summary() {
    let deepseek = load_chat();

    let memory = SummarizationMemory::new(deepseek, 4);
    let mut agent = AiAgent::new(memory, "You are a helpful AI assistant.");

    agent.chat("I'm starting a new company called 'Innovatech'. Our focus is on sustainable energy.").await.unwrap();
    agent.chat("Our first product will be a smart solar panel, codenamed 'Project Helios'.").await.unwrap();
    agent.chat("The marketing budget is set at $50,000.").await.unwrap();
    agent.chat("What is the name of my company and its first product?").await.unwrap();   
}

#[tokio::test]
async fn test_sliding_retrieval() {
    init_tracing(LevelFilter::DEBUG);
    let (openai, model) = load_embed();

    let memory = RetrievalMemory::new(openai, model, 2);
    let mut agent = AiAgent::new(memory, "You are a helpful AI assistant.");
    
    agent.chat("I am planning a vacation to Japan for next spring.").await.unwrap();   
    agent.chat("For my software project, I'm using the React framework for the frontend.").await.unwrap();   
    agent.chat("I want to visit Tokyo and Kyoto while I'm on my trip.").await.unwrap();   
    agent.chat("The backend of my project will be built with Django.").await.unwrap();   
    agent.chat("What cities am I planning to visit on my vacation?").await.unwrap();   
}