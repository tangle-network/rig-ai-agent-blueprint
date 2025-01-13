use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Markdown,
    Other(String),
}

impl Language {
    pub fn from_path(path: &PathBuf) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => Language::Rust,
            Some("ts" | "tsx") => Language::TypeScript,
            Some("js" | "jsx") => Language::JavaScript,
            Some("py") => Language::Python,
            Some("go") => Language::Go,
            Some("md" | "mdx") => Language::Markdown,
            Some(ext) => Language::Other(ext.to_string()),
            None => Language::Other("".to_string()),
        }
    }

    pub fn get_doc_prompt(&self) -> String {
        match self {
            Language::Rust => include_str!("prompts/rust_docs.txt").to_string(),
            Language::TypeScript | Language::JavaScript => {
                include_str!("prompts/typescript_docs.txt").to_string()
            }
            Language::Python => include_str!("prompts/python_docs.txt").to_string(),
            Language::Go => include_str!("prompts/go_docs.txt").to_string(),
            _ => include_str!("prompts/general_docs.txt").to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionAction {
    /// Apply change directly in a commit
    Commit,
    /// Create a review comment with suggestion
    ReviewComment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub original_text: String,
    pub suggested_text: String,
    pub suggestion_type: SuggestionType,
    pub explanation: String,
    pub action: SuggestionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    Spelling,
    Documentation,
    Style,
    Grammar,
}

impl std::fmt::Display for SuggestionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SuggestionType::Spelling => "Spelling",
                SuggestionType::Documentation => "Documentation",
                SuggestionType::Style => "Style",
                SuggestionType::Grammar => "Grammar",
            }
        )
    }
}
