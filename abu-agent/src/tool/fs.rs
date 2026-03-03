#[abu_macros::tool(
    struct_name = FileCreator,
    description = "Create a new file",
)]
pub fn create_file(filepath: &str) -> std::io::Result<()> {
    match std::fs::File::create(filepath) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[abu_macros::tool(
    struct_name = FileWritor,
    description = 
r#"Write content to file.
If the file already exists, the original content will be overwritten directly."#,
)]
pub fn write_file(filepath: &str, content: &str) -> std::io::Result<()> {
    std::fs::write(filepath, content)
}

#[abu_macros::tool(
    struct_name = FileReader,
    description = "Read file and return its content",
)]
pub fn read_file(filepath: &str) -> std::io::Result<String> {
    std::fs::read_to_string(filepath)
}