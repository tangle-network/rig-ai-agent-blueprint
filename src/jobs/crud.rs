use crate::ServiceContext;
use gadget_sdk::event_listener::tangle::jobs::services_post_processor;
use gadget_sdk::{
    self as sdk,
    event_listener::tangle::{jobs::services_pre_processor, TangleEventListener},
    tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled,
};
use rig::providers::anthropic::CLAUDE_3_5_SONNET;
use std::convert::Infallible;

use super::AgentConfiguration;

#[sdk::job(
    id = 0,
    params(configuration),
    result(_),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
        post_processor = services_post_processor,
    ),
)]
pub async fn create_agent(
    configuration: Vec<u8>,
    context: ServiceContext,
) -> Result<String, Infallible> {
    println!("Creating agent");
    let agent_config: AgentConfiguration = serde_json::from_slice(&configuration).unwrap();
    println!("Agent configuration: {:?}", agent_config);
    let client = context.client.lock().await;
    let agent = client
        .agent(CLAUDE_3_5_SONNET)
        .preamble(&agent_config.preamble.unwrap_or_default())
        .temperature(agent_config.temperature.unwrap_or_default())
        .max_tokens(agent_config.max_tokens.unwrap_or_default())
        .build();
    let mut agents = context.agents.lock().await;
    let agent_id = agents.len();
    println!("Agent ID: {}", agent_id);
    agents.push(agent);
    Ok(format!("Agent created with ID: {}", agent_id))
}

/// Removes an agent at the specified index to free up resources
#[sdk::job(
    id = 1,
    params(agent_id),
    result(_),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
        post_processor = services_post_processor,
    ),
)]
pub async fn remove_agent(agent_id: u32, context: ServiceContext) -> Result<String, Infallible> {
    let mut agents = context.agents.lock().await;

    if agent_id as usize >= agents.len() {
        return Ok("".to_string());
    }

    agents.remove(agent_id as usize);
    Ok(format!("Successfully removed agent with ID: {}", agent_id))
}
