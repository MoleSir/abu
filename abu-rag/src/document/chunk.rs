use std::convert::Infallible;
use super::{Chunk, Document, Id};

pub trait DocumentChunk {
    type Error: std::error::Error + 'static + Send + Sync;
    fn chunk(&self, doc: &Document) -> Result<Vec<Chunk>, Self::Error>;
}

pub struct FixedChunker {
    pub chunk_size: usize,
    pub overlap: usize,
}

impl FixedChunker {
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self { chunk_size, overlap }
    }

    pub fn without_overlap(chunk_size: usize) -> Self {
        Self::new(chunk_size, 0)
    }
}

impl DocumentChunk for FixedChunker {
    type Error = Infallible;
    fn chunk(&self, doc: &Document) -> Result<Vec<Chunk>, Self::Error> {
        let mut chunks = vec![];
        let mut start = 0;
        let chars: Vec<char> = doc.text.chars().collect();

        while start < chars.len() {
            let end = (start + self.chunk_size).min(chars.len());
            let chunk_text: String = chars[start..end].iter().collect();

            let chunk = Chunk {
                id: Id::uuid(),
                document_id: doc.id.clone(),
                text: chunk_text,
                start, end 
            };
            chunks.push(chunk);

            if end == chars.len() {
                break;
            }
            start += self.chunk_size - self.overlap;
        }

        Ok(chunks)
    }
}

pub struct ParagraphChunker;

impl DocumentChunk for ParagraphChunker {
    type Error = Infallible;
    fn chunk(&self, doc: &Document) -> Result<Vec<Chunk>, Self::Error> {
        let mut chunks = vec![];
        let mut current_offset = 0;

        for (i, para) in doc.text.split("\n\n").enumerate() {
            let start = current_offset;
            let end = start + para.len();

            let chunk = Chunk {
                id: Id::new(format!("{}_{}", doc.id, i)),
                document_id: doc.id.clone(),
                text: para.to_string(),
                start, end 
            };
            chunks.push(chunk);
        
            current_offset = end + 2; 
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_id_generation() {
        let id1 = Id::uuid();
        let id2 = Id::uuid();
        assert_ne!(id1, id2, "两次生成的 UUID 不应该相同");
        assert!(!id1.to_string().contains("00000000-0000"), "不应该是全 0 的 UUID");
    }

    #[test]
    fn test_fixed_chunker_basic() {
        let doc = Document {
            id: Id::new("doc_1"),
            source: PathBuf::from("dummy.txt"),
            text: "1234567890".to_string(), // 10个字符
        };
        
        let chunker = FixedChunker::new(4, 2);
        let chunks = chunker.chunk(&doc).unwrap();

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].text, "1234");
        assert_eq!(chunks[1].text, "3456");
        assert_eq!(chunks[3].text, "7890");
    }

    #[test]
    fn test_fixed_chunker_utf8_chinese() {
        let doc = Document {
            id: Id::new("doc_2"),
            source: PathBuf::from("dummy.txt"),
            text: "你好世界，这是一个测试".to_string(), // 11 个字符
        };

        let chunker = FixedChunker::new(5, 2);
        let chunks = chunker.chunk(&doc).unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "你好世界，");
        assert_eq!(chunks[1].text, "界，这是一");
        assert_eq!(chunks[2].text, "是一个测试");
    }

    // ---------------------------------------------------------
    // 3. 测试 ParagraphChunker (按段落切分)
    // ---------------------------------------------------------
    #[test]
    fn test_paragraph_chunker() {
        let text = "段落一\n\n段落二的内容\n\n段落三";
        let doc = Document {
            id: Id::new("doc_3"),
            source: PathBuf::from("dummy.txt"),
            text: text.to_string(),
        };

        let chunker = ParagraphChunker;
        let chunks = chunker.chunk(&doc).unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "段落一");
        assert_eq!(chunks[1].text, "段落二的内容");
        assert_eq!(chunks[2].text, "段落三");

        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 9);
        assert_eq!(chunks[1].start, 11);
        assert_eq!(chunks[1].end, 11 + "段落二的内容".len());
    }
}