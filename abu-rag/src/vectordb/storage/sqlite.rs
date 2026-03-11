use std::{borrow::Cow, marker::PhantomData, path::Path, sync::Mutex};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use crate::vectordb::VectorId;
use super::VectorStorage;

#[derive(Debug, thiserror::Error)]
pub enum SqliteStorageError {
    #[error("SQLite database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct SqliteStorage<P> {
    conn: Mutex<Connection>,
    _marker: PhantomData<P>,
}

impl<P: Clone + 'static + Send + Sync> SqliteStorage<P> {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self, SqliteStorageError> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS vector_store (
                id INTEGER PRIMARY KEY,
                payload TEXT NOT NULL
            )",
            []
        )?;

        Ok(Self { conn: Mutex::new(conn), _marker: PhantomData })
    }
}

impl<P> VectorStorage for SqliteStorage<P> 
where 
    P: Clone + 'static + Send + Sync + Serialize + DeserializeOwned
{
    type Error = SqliteStorageError;
    type Payload = P;

    async fn add(&mut self, id: VectorId, payload: Self::Payload) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        let payload_str = serde_json::to_string(&payload)?;

        conn.execute(
            "INSERT OR REPLACE INTO vector_store (id, payload) VALUES (?1, ?2)",
            params![id, payload_str],
        )?;

        Ok(())
    }

    async fn add_batch<IS, PS>(&mut self, ids: IS, payloads: PS) -> Result<(), Self::Error>
        where 
            IS: IntoIterator<Item = VectorId>,
            PS: IntoIterator<Item = Self::Payload>, 
    {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT OR REPLACE INTO vector_store (id, payload) VALUES (?1, ?2)")?;
            for (id, payload) in ids.into_iter().zip(payloads.into_iter()) {
                let payload_str = serde_json::to_string(&payload)?;
                stmt.execute(params![id, payload_str])?;
            }
        }
        tx.commit()?;

        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM vector_store WHERE id = ?1", params![id])?;
        Ok(())
    }

    async fn delete_batch<IS>(&mut self, ids: IS) -> Result<(), Self::Error> 
        where 
            IS: IntoIterator<Item = VectorId>, 
    {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("DELETE FROM vector_store WHERE id = ?1")?;
            for id in ids {
                stmt.execute(params![id])?;
            }
        }
        tx.commit()?;

        Ok(())   
    }

    async fn get(&self, id: VectorId) -> Result<Option<Cow<Self::Payload>>, Self::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT payload FROM vector_store WHERE id = ?1")?;
        if let Some(row) = stmt.query(params![id])?.next()? {
            let payload_str: String = row.get(0)?;
            let payload: P = serde_json::from_str(&payload_str)?;
            Ok(Some(Cow::Owned(payload)))
        } else {
            Ok(None)
        }
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM vector_store", [])?;
        Ok(())
    }
}
