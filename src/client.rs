use serde::Serialize;
use serde::Serializer;
use std::collections::{BTreeMap, HashMap};

use crate::handlers::IpmiState;

#[derive(Debug, Clone, Serialize)]
pub struct ClientError {
    message: String,
    reason: String,
}

impl ClientError {
    pub fn unknown() -> Self {
        Self {
            message: "Unknown error".to_owned(),
            reason: "Unknown".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
// #[serde(untagged)]
pub enum ClientResult<T> {
    Ok(T),
    Err(ClientError),
}

impl<T> std::convert::From<anyhow::Result<T>> for ClientResult<T> {
    fn from(result: anyhow::Result<T>) -> Self {
        match result {
            Ok(value) => ClientResult::Ok(value),
            Err(err) => ClientResult::Err(ClientError {
                message: err.to_string(),
                reason: err.root_cause().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostClientState {
    pub comin: Option<ClientResult<crate::comin::client::State>>,
    pub headscale: Option<ClientResult<Option<crate::headscale::client::NodeState>>>,
    pub ipmi: Option<IpmiState>,
}

/// For use with serde's [serialize_with] attribute
pub fn ordered_map<S, K: Ord + Serialize, V: Serialize>(
    value: &HashMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}
