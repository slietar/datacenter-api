use std::{collections::HashMap, sync::Arc};

use crate::manager::HostManager;

#[derive(Debug, Clone)]
pub struct State {
    pub host_managers: Arc<HashMap<String, HostManager>>,
}
