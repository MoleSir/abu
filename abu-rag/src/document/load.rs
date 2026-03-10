use std::path::Path;
use super::{Document, Id};

pub trait DocumentLoad {
    type Error: std::error::Error + 'static + Send + Sync;
    fn load<P>(&self, path: P) -> Result<Document, Self::Error> 
    where 
        P: AsRef<Path>;
}

pub struct TextLoader;

impl DocumentLoad for TextLoader {
    type Error = std::io::Error;
    
    fn load<P>(&self, path: P) -> Result<Document, Self::Error> 
    where 
        P: AsRef<Path>
    {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        Ok(Document { id: Id::uuid(), source: path.to_path_buf(), text })    
    }
}