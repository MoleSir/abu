//! Readline input with history, completion, highlighting, and line editing.

use anyhow::Context;
use rustyline::{
    completion::{extract_word, Completer, FilenameCompleter, Pair},
    highlight::{CmdKind, Highlighter},
    hint::Hinter,
    validate::{self, ValidationResult, Validator},
    Context as RlContext, Editor, Result as RlResult,
};

const HISTORY_FILENAME: &str = ".history";

// ============================================================================
// Editor setup
// ============================================================================

pub fn create_editor(data_dir: &std::path::Path) -> anyhow::Result<Editor<AbuHelper, rustyline::history::FileHistory>> {
    let history_path = data_dir.join(HISTORY_FILENAME);

    let mut editor = Editor::<AbuHelper, _>::with_history(
        rustyline::Config::default(),
        rustyline::history::FileHistory::new()
    )?;
    editor.set_helper(Some(AbuHelper::new()));

    if history_path.exists() {
        editor.load_history(&history_path)
            .with_context(|| format!("Failed to load history from {:?}", history_path))?;
    }

    Ok(editor)
}

pub fn save_history(editor: &mut Editor<AbuHelper, rustyline::history::FileHistory>, data_dir: &std::path::Path) -> anyhow::Result<()> {
    let history_path = data_dir.join(HISTORY_FILENAME);
    editor.save_history(&history_path)
        .with_context(|| format!("Failed to save history to {:?}", history_path))?;
    Ok(())
}

// ============================================================================
// Helper
// ============================================================================

pub struct AbuHelper {
    file_completer: FilenameCompleter,
}

impl rustyline::Helper for AbuHelper {}

impl AbuHelper {
    fn new() -> Self {
        Self {
            file_completer: FilenameCompleter::new(),
        }
    }
}

// ============================================================================
// Completer — slash commands at input start, file paths elsewhere
// ============================================================================

impl Completer for AbuHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &RlContext<'_>,
    ) -> RlResult<(usize, Vec<Pair>)> {
        let (word_start, word) = extract_word(line, pos, None, |c| {
            c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.'
        });

        // At the start of input, complete slash commands from the registry
        if word_start == 0 && (word.starts_with('/') || word.is_empty()) {
            let all = crate::ui::command::all_commands();
            let completions: Vec<Pair> = all
                .iter()
                .filter(|info| info.name().starts_with(word) || word.is_empty())
                .map(|info| Pair {
                    display: format!("{} - {}", info.name(), info.description()),
                    replacement: format!("{} ", info.name()),
                })
                .collect();
            return Ok((word_start, completions));
        }

        // File path completion
        let (file_start, completions) = self.file_completer.complete(line, pos, _ctx)?;
        Ok((file_start, completions))
    }
}

// ============================================================================
// Highlighter
// ============================================================================

impl Highlighter for AbuHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        std::borrow::Cow::Owned(format!("\x1b[2m{}\x1b[0m", hint))
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> std::borrow::Cow<'b, str> {
        std::borrow::Cow::Borrowed(prompt)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        false
    }
}

// ============================================================================
// Hinter — brief hint for empty input or slash
// ============================================================================

impl Hinter for AbuHelper {
    type Hint = String;

    fn hint(&self, line: &str, _pos: usize, _ctx: &RlContext<'_>) -> Option<Self::Hint> {
        if line == "/" {
            let mut command_hint: String = "\n".to_string();
            for command in crate::ui::command::all_commands() {
                command_hint.push_str(&format!("{} - {}\n", command.name(), command.description()));
            }
            return Some(command_hint);
        }
        None
    }
}

// ============================================================================
// Validator — multi-line continuation on backslash
// ============================================================================

impl Validator for AbuHelper {
    fn validate(
        &self,
        _ctx: &mut validate::ValidationContext,
    ) -> RlResult<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }

    fn validate_while_typing(&self) -> bool {
        false
    }
}
