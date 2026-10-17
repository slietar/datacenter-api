use serde;
use serde::Serialize;
use std::collections::HashMap;

use crate::client::{ClientError, ClientResult};
use crate::headscale::manager::HeadscaleReading;
use crate::live_state::Fallible;

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Node {
    #[ts(type = "number | null")]
    expiration_time: Option<u64>,
    given_name: String,
    id: String,
    ip_addresses: Vec<String>,
    #[ts(type = "number")]
    last_visibility_time: u64,
    machine_key: String,
    name: String,
    node_key: String,
    online: bool,
    tags: Vec<String>,
    user_id: String,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    display_name: String,
    email: String,
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(serialize_with = "crate::client::ordered_map")]
    nodes: HashMap<String, Node>,
    #[serde(serialize_with = "crate::client::ordered_map")]
    users: HashMap<String, User>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NodeState {
    pub node: Node,
    pub user: User,
}

impl NodeState {
    /// Finds the node whose IP addresses include `address` in `reading`, if
    /// any.
    pub fn from_reading(reading: &HeadscaleReading, address: &str) -> ClientResult<Option<Self>> {
        let find = |nodes: &crate::headscale::node::NodesResponse| {
            nodes
                .nodes
                .iter()
                .find(|node| node.ip_addresses.iter().any(|ip| ip == address))
                .map(|node| NodeState {
                    node: Node::from(node),
                    user: User::from(&node.user),
                })
        };

        ClientResult::from_reading(reading, |nodes| find(nodes))
    }
}

impl std::convert::From<&crate::headscale::node::Node> for Node {
    fn from(node: &crate::headscale::node::Node) -> Self {
        Node {
            expiration_time: node.expiry.as_ref().map(|expiry| {
                chrono::DateTime::parse_from_rfc3339(expiry)
                    .unwrap()
                    .timestamp() as u64
            }),
            given_name: node.given_name.clone(),
            id: node.id.clone(),
            ip_addresses: node.ip_addresses.clone(),
            last_visibility_time: node
                .last_seen
                .as_ref()
                .map(|last_seen| {
                    chrono::DateTime::parse_from_rfc3339(last_seen)
                        .unwrap()
                        .timestamp() as u64
                })
                .unwrap_or(0),
            machine_key: node.machine_key.clone(),
            name: node.name.clone(),
            node_key: node.node_key.clone(),
            online: node.online,
            tags: node.tags.clone(),
            user_id: node.user.id.clone(),
        }
    }
}

impl std::convert::From<&crate::headscale::node::User> for User {
    fn from(user: &crate::headscale::node::User) -> Self {
        User {
            display_name: user.display_name.clone(),
            email: user.email.clone(),
            id: user.id.clone(),
            name: user.name.clone(),
        }
    }
}

impl std::convert::From<&crate::headscale::node::NodesResponse> for State {
    fn from(nodes_response: &crate::headscale::node::NodesResponse) -> Self {
        let mut nodes = HashMap::new();
        let mut users = HashMap::new();

        for node in &nodes_response.nodes {
            users.insert(node.user.id.clone(), User::from(&node.user));
            nodes.insert(node.id.clone(), Node::from(node));
        }

        State { nodes, users }
    }
}
