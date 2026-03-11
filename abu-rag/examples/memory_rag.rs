use abu_provider::{chat::{ChatMessage, ChatRequestBuilder}, openai::OpenAi, ChatProvide};
use abu_rag::{document::{DocumentProcessor, ParagraphChunker, TextLoader}, embed::Embedder, vectordb::{FlatCosineIndex, InMemoryStorage, VectorDB, VectorId}};

#[tokio::main] 
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("Error: {e}")
    }
}

async fn result_main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let doc_process = DocumentProcessor::new(TextLoader, ParagraphChunker);
    let embed_model = std::env::var("EMBED_MODEL")?;
    let chat_model = std::env::var("CHAT_MODEL")?;
    let openai = OpenAi::from_env()?;
    let embedder = Embedder::new(openai.clone(), embed_model);
    let index = FlatCosineIndex::new();
    let storgae = InMemoryStorage::<String>::new();
    let mut vectordb = VectorDB::new(index, storgae);

    // build rag
    let chunks = doc_process.process("./document/lhc.md")?;    
    let embeded_chunks = embedder.embed_chunks(chunks).await?;
    for (i, embeded_chunk) in embeded_chunks.into_iter().enumerate() {
        vectordb.add(i as VectorId, embeded_chunk.embedding, embeded_chunk.chunk.text).await?;
    }

    // use rag
    let query = "令狐冲领悟了什么魔法？";
    
    // build rag content
    let query_embeding = embedder.embed_text(query).await?;
    let retr_chunks = vectordb.search(&query_embeding, 3).await?;
    let rag_content = retr_chunks.iter()
        .map(|c| c.1.as_ref().as_str())
        .collect::<Vec<_>>()
        .join("\n");

    println!("{}", rag_content);
    
    // ask llm 
    let request = ChatRequestBuilder::default()
        .model(chat_model)
        .messages(vec![
            ChatMessage::system(format!("Please answer user's question according to context\n\nContext:{rag_content}")),
            ChatMessage::user(query)
        ])
        .temperature(0.5)
        .build()?;
    let response = openai.chat(&request).await?;    

    println!("LLM: {}", response.message.content);

    Ok(())
}