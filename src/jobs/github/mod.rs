use crate::ServiceContext;
use gadget_sdk::{
    self as sdk,
    event_listener::tangle::{
        jobs::{services_post_processor, services_pre_processor},
        TangleEventListener,
    },
    tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

mod file_processor;
mod processor;
mod types;

pub use processor::GithubProcessor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfiguration {
    pub repo_url: String,
    pub branch: String,
    pub agent_id: u32,
}

/// Queries a GitHub repo, finds spelling errors, improve documentation, creates a PR with fixes.
#[sdk::job(
    id = 2,
    params(repo_info),
    result(_),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
        post_processor = services_post_processor,
    ),
)]
pub async fn process_github_repo(
    repo_info: Vec<u8>,
    context: ServiceContext,
) -> Result<String, Infallible> {
    println!("Processing GitHub repo");
    let config = match serde_json::from_slice::<RepoConfiguration>(&repo_info) {
        Ok(config) => config,
        Err(_) => return Ok("Failed to parse repository configuration".to_string()),
    };

    println!("Creating GitHub processor {:?}", config);
    let processor = match GithubProcessor::new(
        context,
        config.repo_url,
        config.branch,
        config.agent_id,
    )
    .await
    {
        Ok(processor) => processor,
        Err(_) => return Ok("Failed to initialize GitHub processor".to_string()),
    };

    println!("Processing repository");
    let suggestions = match processor.process_repository().await {
        Ok(suggestions) => suggestions,
        Err(_) => return Ok("Failed to process repository".to_string()),
    };

    println!(
        "Processed repository with {} suggestions",
        suggestions.len()
    );
    Ok(format!(
        "Processed repository with {} suggestions",
        suggestions.len()
    ))
}
