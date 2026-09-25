use crate::protobuf;
use serde::Serialize;
use std::collections::HashMap;

pub type StateReading = Option<State>;

// #[derive(Debug, Clone, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct Builder {
//     pub building: bool,
//     pub evaluating: bool,
//     pub generation_uuid: String,
// }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployer {
    pub booted_deployment_uuid: Option<String>,
    pub deploying: bool,
    #[serde(serialize_with = "crate::client::ordered_map")]
    pub deployments: HashMap<String, Deployment>,
    pub switched_deployment_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fetcher {
    pub fetching: bool,
    pub status: Option<FetcherStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetcherStatus {
    branch_error_message: Option<String>,
    branch_name: String,
    commit_message: String,
    commit_sha: String,
    error_message: Option<String>,
    remote_fetch_time: Option<u64>,
    remote_name: String,
    remote_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub end_time: Option<u64>,
    pub start_time: Option<u64>,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    // builder: Builder,
    deployer: Deployer,
    fetcher: Fetcher,
}

impl std::convert::From<&protobuf::State> for State {
    fn from(state: &protobuf::State) -> Self {
        let store = state.store.as_ref();

        State {
            deployer: Deployer {
                booted_deployment_uuid: store
                    .map(|store| store.deployment_booted.clone())
                    .filter(|uuid| !uuid.is_empty()),
                deploying: state
                    .deployer
                    .as_ref()
                    .map(|deployer| deployer.is_deploying.unwrap_or(false))
                    .unwrap_or(false),
                deployments: store
                    .map(|store| {
                        store
                            .deployments
                            .iter()
                            .map(|deployment| (deployment.uuid.clone(), deployment.into()))
                            .collect()
                    })
                    .unwrap_or_default(),
                switched_deployment_uuid: store
                    .map(|store| store.deployment_switched.clone())
                    .filter(|uuid| !uuid.is_empty()),
            },
            fetcher: state
                .fetcher
                .as_ref()
                .map(Fetcher::from)
                .unwrap_or(Fetcher {
                    fetching: false,
                    status: None,
                }),
        }
    }
}

impl std::convert::From<&protobuf::Deployment> for Deployment {
    fn from(deployment: &protobuf::Deployment) -> Self {
        Deployment {
            end_time: deployment
                .ended_at
                .as_ref()
                .map(|timestamp| (timestamp.seconds * 1000) as u64),
            start_time: deployment
                .started_at
                .as_ref()
                .map(|timestamp| (timestamp.seconds * 1000) as u64),
            uuid: deployment.uuid.clone(),
        }
    }
}

impl std::convert::From<&protobuf::Fetcher> for Fetcher {
    fn from(f: &protobuf::Fetcher) -> Self {
        Fetcher {
            fetching: f.is_fetching.unwrap_or(false),
            status: f.status.as_ref().map(|status| {
                let protobuf::fetcher::Status::GitRepositoryStatus(status) = status;
                status.into()
            }),
        }
    }
}

impl std::convert::From<&protobuf::GitRepositoryStatus> for FetcherStatus {
    fn from(status: &protobuf::GitRepositoryStatus) -> Self {
        let remote = status
            .remotes
            .iter()
            .find(|remote| remote.name == status.selected_remote_name)
            .unwrap();

        let branch = [remote.main.as_ref(), remote.testing.as_ref()]
            .into_iter()
            .flatten()
            .find(|branch| branch.name == status.selected_branch_name)
            .unwrap();

        FetcherStatus {
            branch_error_message: (!branch.error_msg.is_empty()).then(|| branch.error_msg.clone()),
            branch_name: status.selected_branch_name.clone(),
            commit_message: status.selected_commit_msg.clone(),
            commit_sha: status.selected_commit_id.clone(),
            error_message: (!status.error_msg.is_empty()).then(|| status.error_msg.clone()),
            remote_fetch_time: remote
                .fetched_at
                .as_ref()
                .map(|timestamp| (timestamp.seconds * 1000) as u64),
            remote_name: status.selected_remote_name.clone(),
            remote_url: remote.url.clone(),
        }
    }
}
