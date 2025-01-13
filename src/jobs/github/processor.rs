use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use gadget_sdk::parking_lot::Mutex;
use git2::{Repository, Signature};
use octocrab::models::pulls::PullRequest;
use rig::agent::Agent;
use rig::providers::anthropic::completion::CompletionModel;
use std::{env, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tokio::fs;
use url::Url;

use super::{
    file_processor::FileProcessor,
    types::{Suggestion, SuggestionAction},
};
use crate::ServiceContext;

/// Manages the processing of a GitHub repository for documentation improvements
pub struct GithubProcessor {
    context: ServiceContext,
    agent_id: u32,
    repo_owner: String,
    repo_name: String,
    file_processor: FileProcessor,

    // Git-related fields
    repo: Arc<Mutex<Repository>>,
    branch: String,
    local_dir: TempDir,
}

impl GithubProcessor {
    const MAX_CONCURRENT_FILES: usize = 5;

    pub async fn new(
        context: ServiceContext,
        repo_url: String,
        branch: String,
        agent_id: u32,
    ) -> Result<Self> {
        let (owner, name) = Self::parse_github_url(&repo_url)?;
        let local_dir = tempfile::tempdir()?;
        let repo = Repository::clone(&repo_url, local_dir.path())?;

        Ok(Self {
            context,
            agent_id,
            repo_owner: owner,
            repo_name: name,
            file_processor: FileProcessor::new()?,
            repo: Arc::new(Mutex::new(repo)),
            branch,
            local_dir,
        })
    }

    fn parse_github_url(repo_url: &str) -> Result<(String, String)> {
        let url = Url::parse(repo_url)?;
        let segments: Vec<&str> = url.path_segments().context("Invalid GitHub URL")?.collect();

        match segments.as_slice() {
            [owner, name, ..] => Ok((owner.to_string(), name.to_string())),
            _ => anyhow::bail!("Invalid GitHub repository URL format"),
        }
    }

    pub async fn process_repository(&self) -> Result<Vec<Suggestion>> {
        gadget_sdk::info!("Starting repository processing...");
        let agents = self.context.agents.lock().await;
        let agent = agents
            .get(self.agent_id as usize)
            .context("Agent not found")?;

        self.prepare_repository().await?;
        let branch_name = self.create_feature_branch()?;
        gadget_sdk::info!("Created feature branch: {}", branch_name);

        let suggestions = self.process_files(&agent).await?;

        if !suggestions.is_empty() {
            self.create_github_pr(&suggestions, &branch_name).await?;
        }

        Ok(suggestions)
    }

    async fn prepare_repository(&self) -> Result<()> {
        let repo = self.repo.lock();
        let mut remote = repo.find_remote("origin")?;
        remote.fetch(&[&self.branch], None, None)?;
        Ok(())
    }

    fn create_feature_branch(&self) -> Result<String> {
        let repo = self.repo.lock();
        let branch_name = format!("ai-docs-improvements-{}", chrono::Utc::now().timestamp());

        let commit = repo.head()?.peel_to_commit()?;
        repo.branch(&branch_name, &commit, false)?;

        let obj = repo.revparse_single(&branch_name)?;
        repo.checkout_tree(&obj, None)?;
        repo.set_head(&format!("refs/heads/{}", branch_name))?;

        Ok(branch_name)
    }

    async fn commit_file_changes(&self, file: &PathBuf, suggestions: &[Suggestion]) -> Result<()> {
        let commit_msg = self.generate_file_commit_message(file, suggestions);
        let repo = self.repo.lock();

        let signature = Signature::now("AI Assistant", "ai@example.com")?;
        let tree_id = repo.index()?.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent = repo.head()?.peel_to_commit()?;

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &commit_msg,
            &tree,
            &[&parent],
        )?;

        Ok(())
    }

    async fn process_files(&self, agent: &Agent<CompletionModel>) -> Result<Vec<Suggestion>> {
        let files = self.scan_repository().await?;
        gadget_sdk::info!("Found {} files to process", files.len());

        let pending_commits = Arc::new(Mutex::new(Vec::new()));
        let review_suggestions = Arc::new(Mutex::new(Vec::new()));

        self.process_files_concurrently(files, agent, &pending_commits, &review_suggestions)
            .await?;

        let (commit_suggestions, review_suggestions) = self
            .handle_pending_commits(pending_commits, review_suggestions)
            .await?;

        Ok(commit_suggestions
            .into_iter()
            .chain(review_suggestions)
            .collect())
    }

    async fn scan_repository(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut stack = vec![self.local_dir.path().to_path_buf()];

        while let Some(current_path) = stack.pop() {
            let mut entries = fs::read_dir(&current_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push(path);
                }
            }
        }

        Ok(files)
    }

    async fn process_files_concurrently(
        &self,
        files: Vec<PathBuf>,
        agent: &Agent<CompletionModel>,
        pending_commits: &Arc<Mutex<Vec<(PathBuf, Vec<Suggestion>)>>>,
        review_suggestions: &Arc<Mutex<Vec<Suggestion>>>,
    ) -> Result<()> {
        stream::iter(files)
            .map(|file| {
                let pending_commits = Arc::clone(pending_commits);
                let review_suggestions = Arc::clone(review_suggestions);

                async move {
                    gadget_sdk::info!("Processing file: {}", file.display());
                    match self.file_processor.process_file(&file, agent).await {
                        Ok(suggestions) if !suggestions.is_empty() => {
                            let (commits, reviews): (Vec<_>, Vec<_>) = suggestions
                                .into_iter()
                                .partition(|s| matches!(s.action, SuggestionAction::Commit));

                            if !commits.is_empty() {
                                gadget_sdk::info!(
                                    "Found {} commit suggestions for {}",
                                    commits.len(),
                                    file.display()
                                );
                                pending_commits.lock().push((file.clone(), commits));
                            }
                            if !reviews.is_empty() {
                                gadget_sdk::info!(
                                    "Found {} review suggestions for {}",
                                    reviews.len(),
                                    file.display()
                                );
                                review_suggestions.lock().extend(reviews);
                            }
                        }
                        Err(e) => gadget_sdk::error!("Failed to process {}: {}", file.display(), e),
                        _ => gadget_sdk::info!("No suggestions found for {}", file.display()),
                    }
                }
            })
            .buffer_unordered(Self::MAX_CONCURRENT_FILES)
            .collect::<Vec<()>>()
            .await;

        Ok(())
    }

    async fn handle_pending_commits(
        &self,
        pending_commits: Arc<Mutex<Vec<(PathBuf, Vec<Suggestion>)>>>,
        review_suggestions: Arc<Mutex<Vec<Suggestion>>>,
    ) -> Result<(Vec<Suggestion>, Vec<Suggestion>)> {
        let pending_commits = Arc::try_unwrap(pending_commits)
            .expect("All tasks completed")
            .into_inner();

        let mut commit_suggestions = Vec::new();
        for (file, suggestions) in pending_commits {
            gadget_sdk::info!("Committing changes for file: {}", file.display());
            if let Err(e) = self.commit_file_changes(&file, &suggestions).await {
                gadget_sdk::error!("Failed to commit changes for {}: {}", file.display(), e);
            } else {
                commit_suggestions.extend(suggestions);
            }
        }

        let review_suggestions = Arc::try_unwrap(review_suggestions)
            .expect("All tasks completed")
            .into_inner();

        Ok((commit_suggestions, review_suggestions))
    }

    fn generate_file_commit_message(&self, file: &PathBuf, suggestions: &[Suggestion]) -> String {
        let file_path = file.display();
        let improvements = suggestions
            .iter()
            .map(|s| s.suggestion_type.to_string())
            .collect::<std::collections::HashSet<_>>();

        format!(
            "AI: Improve {} documentation\n\nImprovements:\n{}\n\nTotal changes: {}",
            file_path,
            improvements
                .into_iter()
                .map(|i| format!("- {}", i))
                .collect::<Vec<_>>()
                .join("\n"),
            suggestions.len()
        )
    }

    async fn create_github_pr(&self, suggestions: &[Suggestion], branch_name: &str) -> Result<()> {
        gadget_sdk::info!(
            "Creating pull request for {} suggestions",
            suggestions.len()
        );

        let pr = self.create_pull_request(suggestions, branch_name).await?;

        if !suggestions.is_empty() {
            gadget_sdk::info!(
                "Adding {} review comments to PR #{}",
                suggestions.len(),
                pr.number
            );
            self.add_review_comments(pr.number, suggestions).await?;
        }

        Ok(())
    }

    async fn create_pull_request(
        &self,
        suggestions: &[Suggestion],
        branch_name: &str,
    ) -> Result<PullRequest> {
        let pr_title = "AI: Documentation and Style Improvements";
        let pr_body = self.generate_pr_description(suggestions);

        let pr = self
            .context
            .github
            .lock()
            .await
            .pulls(&self.repo_owner, &self.repo_name)
            .create(pr_title, branch_name, "main")
            .body(pr_body)
            .send()
            .await?;

        Ok(pr)
    }

    fn generate_pr_description(&self, suggestions: &[Suggestion]) -> String {
        let mut description =
            String::from("# AI-Assisted Documentation and Style Improvements\n\n");
        let mut current_file = None;

        for suggestion in suggestions {
            if current_file.as_ref() != Some(&suggestion.file_path) {
                current_file = Some(suggestion.file_path.clone());
                description.push_str(&format!("\n## {}\n\n", suggestion.file_path.display()));
            }

            description.push_str(&format!(
                "- Line {}: {}\n  - {}\n",
                suggestion.line_number,
                suggestion.suggestion_type.to_string(),
                suggestion.explanation
            ));
        }

        description
    }

    async fn add_review_comments(&self, pr_number: u64, suggestions: &[Suggestion]) -> Result<()> {
        let token = env::var("GITHUB_TOKEN").context("Missing GITHUB_TOKEN")?;
        let review_id = self.create_review(pr_number, &token).await?;

        for suggestion in suggestions {
            self.create_review_comment(pr_number, review_id, suggestion, &token)
                .await?;
        }

        self.submit_review(pr_number, review_id, &token).await?;
        Ok(())
    }

    async fn create_review(&self, pr_number: u64, token: &str) -> Result<u64> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews",
            self.repo_owner, self.repo_name, pr_number
        );

        let response = client
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&serde_json::json!({
                "body": "AI-assisted documentation review",
                "event": "COMMENT"
            }))
            .send()
            .await?;

        let review: serde_json::Value = response.json().await?;
        Ok(review["id"].as_u64().context("No review ID found")?)
    }

    async fn create_review_comment(
        &self,
        pr_number: u64,
        review_id: u64,
        suggestion: &Suggestion,
        token: &str,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews/{}/comments",
            self.repo_owner, self.repo_name, pr_number, review_id
        );

        let response = client
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&serde_json::json!({
                "body": format!("```suggestion\n{}\n```\n\n{}",
                    suggestion.suggested_text,
                    suggestion.explanation
                ),
                "path": suggestion.file_path.to_string_lossy(),
                "line": suggestion.line_number,
                "side": "RIGHT"
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to create review comment: {}", error);
        }

        Ok(())
    }

    async fn submit_review(&self, pr_number: u64, review_id: u64, token: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews/{}/events",
            self.repo_owner, self.repo_name, pr_number, review_id
        );

        let response = client
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&serde_json::json!({
                "body": "AI-assisted documentation review completed",
                "event": "COMMENT"
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to submit review: {}", error);
        }

        Ok(())
    }
}
