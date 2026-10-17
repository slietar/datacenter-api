use crate::client::{ClientError, ClientResult};
use crate::live_state::Fallible;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpmiState {
    pub power_is_on: bool,
    pub power_restore_policy: String,
    pub sensors: HashMap<String, String>,
}

impl std::convert::From<super::manager::IpmiReading> for ClientResult<IpmiState> {
    fn from(reading: super::manager::IpmiReading) -> Self {
        match reading {
            Fallible::Ok(state) => ClientResult::Ok(state.as_ref().clone()),
            Fallible::Partial { value, .. } => ClientResult::Partial {
                error: ClientError::unknown(),
                value: value.as_ref().clone(),
            },
            Fallible::Err(_) => ClientResult::Err(ClientError::unknown()),
        }
    }
}
