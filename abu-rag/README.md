# abu-rag

Retrieval-Augmented Generation system with document loading, splitting, embedding, and simple vector database operations.



## Features

- Document
  - Document Load (`DocumentLoad` trait)
    - [x] text file
  - Document chunk (`DocumentChunk` trait)
    - [x] fixed size
    - [x] paragraph   
- Embed, use abu-provider to support sentance embeddig
- VectorDB
  - VectorIndex (`VectorIndex` trait)
    - [x] flat
    - [x] random hyperplane LSH
    - [x] inverted file 
  - VectorStorge (`VectorStorage` trait)
    - [x] in memory
    - [x] sqlite



## Examples

```rust
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
```