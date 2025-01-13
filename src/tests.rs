const N: usize = 1;

use blueprint_test_utils::tangle::NodeConfig;
use blueprint_test_utils::test_ext::new_test_ext_blueprint_manager;
use blueprint_test_utils::{
    get_next_call_id, run_test_blueprint_manager, setup_log, submit_job,
    wait_for_completion_of_tangle_job, BoundedVec, InputValue, Job,
};
use gadget_blueprint_serde::to_field;
use gadget_sdk::tangle_subxt::tangle_testnet_runtime::api::runtime_types;
use serde_json::json;

use crate::jobs::github::RepoConfiguration;

#[tokio::test(flavor = "multi_thread")]
async fn test_blueprint() {
    setup_log();
    gadget_sdk::info!("Running AI Agent blueprint test");
    let tmp_dir = blueprint_test_utils::tempfile::TempDir::new().unwrap();
    let tmp_dir_path = tmp_dir.path().to_string_lossy().into_owned();
    let node_config = NodeConfig::new(false);

    new_test_ext_blueprint_manager::<N, 1, String, _, _>(
        tmp_dir_path,
        run_test_blueprint_manager,
        node_config,
    )
    .await
    .execute_with_async(|client, handles, blueprint, _| async move {
        let keypair = handles[0].sr25519_id().clone();
        let service = &blueprint.services[0];
        let service_id = service.id;

        // Create agent configuration
        let agent_config = json!({
            "preamble": "You are a helpful AI assistant focused on code review and spelling corrections.",
            "static_context": [],
            "static_tools": [],
            "additional_params": null,
            "max_tokens": 2000,
            "temperature": 0.7,
            "tools": []
        });

        gadget_sdk::info!("Submitting CREATE_AGENT job with service ID {service_id}");

        let job_args = vec![InputValue::List(BoundedVec(
            serde_json::to_vec(&agent_config)
                .unwrap()
                .iter()
                .map(|v| runtime_types::tangle_primitives::services::field::Field::Uint8(*v)
            ).collect()
        ))];
        let call_id = get_next_call_id(client)
            .await
            .expect("Failed to get next job id")
            .saturating_sub(1);

        let job = submit_job(
            client,
            &keypair,
            service_id,
            Job::from(0), // CREATE_AGENT job ID
            job_args,
            call_id,
        )
        .await
        .expect("Failed to submit job");

        let create_agent_call_id = job.call_id;

        let job_results = wait_for_completion_of_tangle_job(client, service_id, create_agent_call_id, N)
            .await
            .expect("Failed to wait for job completion");

        assert_eq!(job_results.service_id, service_id);
        assert_eq!(job_results.call_id, create_agent_call_id);

        gadget_sdk::info!("Agent created successfully! Moving on to repo processing...");

        // Process GitHub repo
        let repo_config = RepoConfiguration {
            repo_url: "https://github.com/tangle-network/tnt-core".to_string(),
            branch: "main".to_string(),
            agent_id: 0,
        };

        let job_args = vec![
            to_field(serde_json::to_vec(&repo_config).unwrap()).unwrap(),
        ];

        let job = submit_job(
            client,
            &keypair,
            service_id,
            Job::from(2), // PROCESS_GITHUB_REPO job ID
            job_args,
            call_id + 1,
        )
        .await
        .expect("Failed to submit job");

        let process_repo_call_id = job.call_id;
        gadget_sdk::info!(
            "Submitted PROCESS_GITHUB_REPO job with service ID {service_id} has call id {process_repo_call_id}"
        );

        let job_results = wait_for_completion_of_tangle_job(client, service_id, process_repo_call_id, N)
            .await
            .expect("Failed to wait for job completion");

        assert_eq!(job_results.service_id, service_id);
        assert_eq!(job_results.call_id, process_repo_call_id);
        assert!(!job_results.result.is_empty(), "Expected non-empty result from repo processing");
    })
    .await
}
