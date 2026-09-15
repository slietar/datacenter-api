use std::{collections::HashMap, sync::Arc};

use crate::headscale::manager::HeadscaleManager;
use crate::manager::HostManager;

#[derive(Debug, Clone)]
pub struct State {
    pub headscale_manager: Option<HeadscaleManager>,
    pub host_managers: Arc<HashMap<String, HostManager>>,
}
