use color_eyre::Result;
use dotenv::dotenv;
use gadget_sdk as sdk;
use octocrab::Octocrab;
use rig::providers::anthropic;
use rig_ai_agent_blueprint::{
    self as blueprint,
    jobs::{
        crud::{CreateAgentEventHandler, RemoveAgentEventHandler},
        github::ProcessGithubRepoEventHandler,
    },
};
use sdk::runners::tangle::TangleConfig;
use sdk::runners::BlueprintRunner;
use std::{env, sync::Arc};
use tokio::sync::Mutex as TokioMutex;

#[sdk::main(env)]
async fn main() -> Result<()> {
    dotenv().ok();

    println!("Setting up the environment ...");
    let client = anthropic::ClientBuilder::new(
        &env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set"),
    )
    .build();
    println!("Anthropic client created...");
    let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env variable is required");
    let github = Octocrab::builder()
        .personal_token(token)
        .build()
        .expect("Failed to create GitHub client");
    println!("GitHub client created {:?}", github);
    println!("GitHub client created...");
    let context = blueprint::ServiceContext {
        config: env.clone(),
        call_id: None,
        client: Arc::new(TokioMutex::new(client)),
        agents: Arc::new(TokioMutex::new(Vec::new())),
        github: Arc::new(TokioMutex::new(github)),
    };

    // Create the event handler from the job
    let crate_agent_job = CreateAgentEventHandler::new(&env, context.clone()).await?;
    let remove_agent_job = RemoveAgentEventHandler::new(&env, context.clone()).await?;
    let process_repo_job = ProcessGithubRepoEventHandler::new(&env, context).await?;

    tracing::info!("Starting the event watcher ...");
    let tangle_config = TangleConfig::default();
    BlueprintRunner::new(tangle_config, env)
        .job(crate_agent_job)
        .job(remove_agent_job)
        .job(process_repo_job)
        .run()
        .await?;

    tracing::info!("Exiting...");
    Ok(())
}
