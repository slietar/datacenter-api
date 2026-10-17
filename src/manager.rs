use crate::comin;
use crate::comin::manager::CominManager;
use crate::ipmi::manager::{IpmiManager, IpmiReading};

use std::sync::Arc;

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
                    name.to_string(),
                    host_config.address.clone(),
                    config.ipmi.username.clone(),
                    config.ipmi.password.as_ref().unwrap().as_bytes().to_vec(),
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

    pub async fn get_comin_state(&self) -> Option<comin::manager::CominReading> {
        let comin_manager = self.comin_manager.as_ref()?;
        Some(comin_manager.get().await)
    }

    pub async fn get_ipmi_state(&self) -> Option<IpmiReading> {
        let ipmi_manager = self.ipmi_manager.as_ref()?;
        Some(ipmi_manager.get().await)
    }
}
