use gadget_sdk::{self as sdk, contexts::TangleClientContext};
use octocrab::Octocrab;
use rig::{
    agent::Agent,
    providers::anthropic::{completion::CompletionModel, Client},
};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

pub mod jobs;

#[cfg(test)]
pub mod tests;

#[derive(Clone, TangleClientContext)]
pub struct ServiceContext {
    #[config]
    pub config: sdk::config::StdGadgetConfiguration,
    #[call_id]
    pub call_id: Option<u64>,
    pub client: Arc<TokioMutex<Client>>,
    pub agents: Arc<TokioMutex<Vec<Agent<CompletionModel>>>>,
    pub github: Arc<TokioMutex<Octocrab>>,
}
