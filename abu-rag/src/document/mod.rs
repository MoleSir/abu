mod model;
use std::path::Path;

pub use model::*;
mod chunk;
pub use chunk::*;
mod load;
pub use load::*;

pub struct DocumentProcessor<L, C> {
    loader: L,
    chunker: C,
}

impl<L, C> DocumentProcessor<L, C>
where
    L: DocumentLoad,
    C: DocumentChunk,
{
    pub fn new(loader: L, chunker: C) -> Self {
        Self { loader, chunker }
    }

    pub fn process<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Chunk>, DocumentError> {
        let doc = self.loader.load(path)
            .map_err(|e| DocumentError::Load(Box::new(e)))?;
        let chunks = self.chunker.chunk(&doc)
            .map_err(|e| DocumentError::Chunk(Box::new(e)))?;
        Ok(chunks)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("load error: {0}")]
    Load(Box<dyn std::error::Error + 'static + Send + Sync>),

    #[error("chunk error: {0}")]
    Chunk(Box<dyn std::error::Error + 'static + Send + Sync>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::env::temp_dir;

    #[test]
    fn test_processor_end_to_end() {
        // 1. 在系统的临时目录创建一个测试文件
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("rag_test_doc.txt");
        
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Hello Rust!\n\nThis is a RAG test.").unwrap();
        
        // 2. 初始化 Processor
        let processor = DocumentProcessor {
            loader: TextLoader,
            chunker: ParagraphChunker,
        };

        // 3. 执行处理
        let result = processor.process(&file_path);
        assert!(result.is_ok(), "文档处理应该成功");
        
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "Hello Rust!");
        // 注意 writeln! 会在末尾加上 \n，所以第二个段落包含 \n
        assert_eq!(chunks[1].text, "This is a RAG test.\n");

        // 4. 清理临时文件
        std::fs::remove_file(file_path).unwrap();
    }
    
    #[test]
    fn test_processor_file_not_found() {
        let processor = DocumentProcessor {
            loader: TextLoader,
            chunker: FixedChunker::without_overlap(10),
        };

        let result = processor.process(Path::new("not_exist_file_12345.txt"));
        
        assert!(matches!(result, Err(DocumentError::Load(_))));
    }
}