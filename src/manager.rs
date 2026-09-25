use crate::comin;
use crate::comin::client::{State, StateReading};
use crate::comin::manager::CominManager;
use crate::handlers::{IpmiState, read_host_state};
use crate::ipmi::manager::{IpmiManager, IpmiReading};

use ipmi_rs::Ipmi;
use ipmi_rs::rmcp::Rmcp;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Notify, broadcast};

#[derive(Debug)]
pub struct HostManager {
    pub config: crate::config::Config,
    pub host_config: crate::config::Host,
    pub ipmi_manager: Option<Arc<IpmiManager>>,
    pub comin_manager: Option<Arc<CominManager>>,
}

impl HostManager {
    pub fn new(
        name: &str,
        config: &crate::config::Config,
        host_config: &crate::config::Host,
    ) -> Self {
        Self {
            config: config.clone(),
            host_config: host_config.clone(),
            ipmi_manager: host_config.ipmi.then(|| {
                Arc::new(IpmiManager::new(
                    host_config.address.clone(),
                    config.ipmi.username.clone(),
                    config.ipmi.password.as_ref().unwrap().as_bytes().to_vec(),
                    Duration::from_secs(30),
                ))
            }),
            comin_manager: host_config.comin.then(|| {
                Arc::new(CominManager::new(
                    name.to_string(),
                    host_config.address.clone(),
                ))
            }),
        }
    }

    pub async fn get_comin_state(&self) -> Option<Arc<comin::manager::CominStateReading>> {
        let comin_manager = self.comin_manager.as_ref()?;
        let receiver = comin_manager.subscribe().await;
        let state = receiver.borrow();

        Some(state)
    }

    pub async fn get_ipmi_state(&self) -> Option<IpmiReading> {
        let ipmi_manager = self.ipmi_manager.as_ref()?;
        let mut receiver = ipmi_manager.subscribe();

        loop {
            match receiver.recv().await {
                Ok(reading) => return Some((*reading).clone()),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}
