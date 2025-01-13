use super::types::{Language, Suggestion, SuggestionAction};
use anyhow::Result;
use rig::providers::anthropic::completion::CompletionModel;
use rig::{agent::Agent, completion::Prompt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

pub struct FileProcessor {
    ignored_patterns: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct AIResponse {
    suggestions: Vec<Suggestion>,
}

impl FileProcessor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ignored_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "dist".to_string(),
            ],
        })
    }

    const MAX_FILE_SIZE: u64 = 1_000_000; // 1MB
    const CHUNK_SIZE: usize = 500; // Process 500 lines at a time
    const MAX_CHUNKS_PER_FILE: usize = 20; // Maximum number of chunks to process per file

    pub async fn process_file(
        &self,
        path: &PathBuf,
        agent: &Agent<CompletionModel>,
    ) -> Result<Vec<Suggestion>> {
        if self.should_ignore(path) {
            return Ok(vec![]);
        }

        let metadata = fs::metadata(path).await?;
        if metadata.len() > Self::MAX_FILE_SIZE {
            tracing::warn!(
                "Large file detected: {}, processing in chunks",
                path.display()
            );
        }

        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let language = Language::from_path(path);
        let mut all_suggestions = Vec::new();

        // Calculate optimal chunk size based on file size
        let chunk_size = if lines.len() > Self::CHUNK_SIZE * Self::MAX_CHUNKS_PER_FILE {
            lines.len() / Self::MAX_CHUNKS_PER_FILE
        } else {
            Self::CHUNK_SIZE
        };

        // Process file in chunks
        for (chunk_idx, lines_chunk) in lines.chunks(chunk_size).enumerate() {
            let chunk_content = lines_chunk.join("\n");
            let base_line_number = chunk_idx * chunk_size;

            tracing::debug!(
                "Processing {} chunk {}/{}",
                path.display(),
                chunk_idx + 1,
                (lines.len() + chunk_size - 1) / chunk_size
            );

            // Process chunk with retries
            let chunk_suggestions = self
                .process_chunk(path, &language, &chunk_content, base_line_number, agent)
                .await?;

            all_suggestions.extend(chunk_suggestions);
        }

        Ok(all_suggestions)
    }

    async fn process_chunk(
        &self,
        path: &PathBuf,
        language: &Language,
        content: &str,
        base_line_number: usize,
        agent: &Agent<CompletionModel>,
    ) -> Result<Vec<Suggestion>> {
        let doc_prompt = language.get_doc_prompt();
        let prompt = format!(
            "{}\n\nFile: {}\nContent (lines {}-{}):\n{}\n\nProvide suggestions in JSON format.",
            doc_prompt,
            path.display(),
            base_line_number + 1,
            base_line_number + content.lines().count(),
            content
        );

        let response = agent.prompt(&prompt).await?;
        let ai_response: AIResponse = serde_json::from_str(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse AI response: {}", e))?;

        // Adjust line numbers for the chunk
        let mut suggestions = ai_response.suggestions;
        for suggestion in &mut suggestions {
            suggestion.line_number += base_line_number;
        }

        // For each suggestion, ask the agent to decide the action
        for suggestion in &mut suggestions {
            let action = self.decide_suggestion_action(agent, suggestion).await?;
            suggestion.action = action;
        }

        Ok(suggestions)
    }

    async fn decide_suggestion_action(
        &self,
        agent: &Agent<CompletionModel>,
        suggestion: &Suggestion,
    ) -> Result<SuggestionAction> {
        let prompt = format!(
            "Analyze this suggested change and decide whether it should be:\n\
             1. Committed directly (for clear improvements like spelling fixes)\n\
             2. Suggested as a review comment (for substantial or debatable changes)\n\n\
             Change Type: {}\n\
             Original: {}\n\
             Suggestion: {}\n\
             Explanation: {}\n\n\
             Respond with either 'COMMIT' or 'REVIEW' and a brief explanation.",
            suggestion.suggestion_type,
            suggestion.original_text,
            suggestion.suggested_text,
            suggestion.explanation
        );

        let response = agent.prompt(&prompt).await?;

        if response.to_uppercase().contains("COMMIT") {
            Ok(SuggestionAction::Commit)
        } else {
            Ok(SuggestionAction::ReviewComment)
        }
    }

    fn should_ignore(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy();
        self.ignored_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
    }
}
