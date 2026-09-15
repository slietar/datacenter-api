use serde::Serialize;

use crate::protobuf;

#[derive(Serialize)]
pub struct CominState {
    fetcher: Fetcher,
}

#[derive(Serialize)]
pub struct Fetcher {
    pub fetching: bool,
    pub status: Option<FetcherStatus>,
}

#[derive(Serialize)]
pub struct FetcherStatus {
    branch_error_message: Option<String>,
    branch_name: String,
    commit_message: String,
    commit_sha: String,
    error_message: Option<String>,
    remote_fetch_time: u64,
    remote_name: String,
    remote_url: String,
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
            branch_error_message: (!branch.error_msg.is_empty())
                .then(|| branch.error_msg.clone()),
            branch_name: status.selected_branch_name.clone(),
            commit_message: status.selected_commit_msg.clone(),
            commit_sha: status.selected_commit_id.clone(),
            error_message: (!status.error_msg.is_empty()).then(|| status.error_msg.clone()),
            remote_fetch_time: remote
                .fetched_at
                .as_ref()
                .map(|timestamp| timestamp.seconds.max(0) as u64)
                .unwrap_or(0),
            remote_name: status.selected_remote_name.clone(),
            remote_url: remote.url.clone(),
        }
    }
}
