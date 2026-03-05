#[abu_macros::tool(
    struct_name = Terminator,
    description = "Answer the user's question, and terminate the conversation.",
)]
fn terminate(#[arg(description="Answer to the user's question.")] answer: &str) -> String {
    answer.to_string()
}