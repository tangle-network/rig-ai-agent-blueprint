use rig::completion::Document;
use serde::{Deserialize, Serialize};

pub mod crud;
pub mod github;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfiguration {
    /// System prompt
    preamble: Option<String>,
    /// Context documents always available to the agent
    static_context: Vec<Document>,
    /// Tools that are always available to the agent (by name)
    static_tools: Vec<String>,
    /// Additional parameters to be passed to the model
    additional_params: Option<serde_json::Value>,
    /// Maximum number of tokens for the completion
    max_tokens: Option<u64>,
    // /// List of vector store, with the sample number
    // dynamic_context: Vec<(usize, Box<dyn VectorStoreIndexDyn>)>,
    // /// Dynamic tools
    // dynamic_tools: Vec<(usize, Box<dyn VectorStoreIndexDyn>)>,
    /// Temperature of the model
    temperature: Option<f64>,
    /// Actual tool implementations, identifiable by strings (need to map intelligently)
    tools: Vec<String>,
}
