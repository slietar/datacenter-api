use crate::client::ClientResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IpmiState {
    pub power_is_on: bool,
    pub power_restore_policy: String,
    #[serde(serialize_with = "crate::client::ordered_map")]
    pub sensors: HashMap<String, String>,
}

impl std::convert::From<super::manager::IpmiReading> for ClientResult<IpmiState> {
    fn from(reading: super::manager::IpmiReading) -> Self {
        ClientResult::from_reading(&reading, IpmiState::clone)
    }
}
