#![allow(unused)]

use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pxe {
    pub caches: Vec<Url>,
    pub cachix: String,
    pub store: PathBuf,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ipmi {
    pub username: String,
    pub password: Option<String>,
    pub password_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Host {
    pub address: String,
    pub mac: Option<String>,

    pub ipmi: bool,
    pub comin: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub host: HashMap<String, Host>,
    pub ipmi: Ipmi,
    pub pxe: Pxe,
}

impl Config {
    pub fn find_host_by_mac(&self, mac: &str) -> Option<(&String, &Host)> {
        self.host
            .iter()
            .find(|(_, data)| data.mac.as_ref().map(String::as_ref) == Some(mac))
    }
}
