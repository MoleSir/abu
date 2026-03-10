use std::{path::Path, sync::Mutex};
use rusqlite::{params, Connection};
use crate::{document::Chunk, embed::EmbeddedChunk};
use super::{util, ScoredChunk, VectorDB};

#[derive(Debug, thiserror::Error)]
pub enum SqliteDBError {
    #[error("SQLite database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct SqliteVectorDB {
    conn: Mutex<Connection>,
}

impl SqliteVectorDB {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, SqliteDBError> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS vector_store (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk TEXT NOT NULL,
                embedding BLOB NOT NULL
            )",[],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

// 辅助函数：将 f32 切片转换为紧凑的 u8 字节流以便存入 BLOB
fn f32_to_bytes(slice: &[f32]) -> Vec<u8> {
    slice.iter().flat_map(|&f| f.to_ne_bytes()).collect()
}

// 辅助函数：将 u8 字节流还原回 f32 向量
fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes(b.try_into().unwrap()))
        .collect()
}

// 3. 实现 VectorDB Trait
impl VectorDB for SqliteVectorDB {
    type Error = SqliteDBError;

    async fn add_chunks<I>(&mut self, chunks: I) -> Result<(), Self::Error> 
    where 
        I: IntoIterator<Item = EmbeddedChunk>
    {
        let mut conn = self.conn.lock().unwrap();
        
        // 使用事务大幅提升批量插入(Bulk Insert)的性能
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT INTO vector_store (chunk, embedding) VALUES (?1, ?2)")?;
            for ec in chunks {
                let chunk_json = serde_json::to_string(&ec.chunk)?;
                let embedding_bytes = f32_to_bytes(&ec.embedding);
                
                stmt.execute(params![chunk_json, embedding_bytes])?;
            }
        }
        tx.commit()?;
        
        Ok(())
    }

    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<ScoredChunk>, Self::Error> {
        let conn = self.conn.lock().expect("lock conn");
        let mut stmt = conn.prepare("SELECT chunk, embedding FROM vector_store")?;
        let mut rows = stmt.query([])?;
        
        // 利用 SELECT 语句不断获取数据
        let mut scored_chunks: Vec<(Chunk, f32)> = Vec::new();        
        while let Some(row) = rows.next()? {
            let chunk_json: String = row.get(0)?;
            let embedding_bytes: Vec<u8> = row.get(1)?;
            
            // 反序列化还原数据
            let chunk: Chunk = serde_json::from_str(&chunk_json)?;
            let embedding = bytes_to_f32(&embedding_bytes);
            
            // 计算相似度
            let score = util::cosine_similarity(&embedding, query_embedding);
            scored_chunks.push((chunk, score));
        }

        // 降序 / 截断
        scored_chunks.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_chunks.truncate(limit);        
        Ok(
            scored_chunks
                .into_iter()
                .map(|(c, s)| ScoredChunk::new(c, s))
                .collect()
        )
    }
}