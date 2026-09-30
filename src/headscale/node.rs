use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodesResponse {
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub machine_key: String,
    pub node_key: String,
    pub disco_key: String,
    pub ip_addresses: Vec<String>,
    pub name: String,
    pub user: User,
    pub last_seen: Option<String>,
    pub expiry: Option<String>,
    pub pre_auth_key: Option<PreAuthKey>,
    pub created_at: String,
    pub register_method: RegisterMethod,
    pub given_name: String,
    pub online: bool,
    pub approved_routes: Vec<String>,
    pub available_routes: Vec<String>,
    pub subnet_routes: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub display_name: String,
    pub email: String,
    pub provider_id: String,
    pub provider: String,
    pub profile_pic_url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreAuthKey {
    pub user: User,
    pub id: String,
    pub key: String,
    pub reusable: bool,
    pub ephemeral: bool,
    pub used: bool,
    pub expiration: Option<String>,
    pub created_at: String,
    pub acl_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum RegisterMethod {
    #[serde(rename = "REGISTER_METHOD_UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "REGISTER_METHOD_AUTH_KEY")]
    AuthKey,
    #[serde(rename = "REGISTER_METHOD_CLI")]
    Cli,
    #[serde(rename = "REGISTER_METHOD_OIDC")]
    Oidc,
}
