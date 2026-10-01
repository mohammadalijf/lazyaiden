use std::sync::Arc;

use fellow_client::{Device, FellowApi};

use crate::error::{CoreError, Result};

/// A brewer plus whether it is the one currently acted on.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceView {
    /// The brewer.
    pub device: Device,
    /// Whether device-scoped calls act on it.
    pub active: bool,
}

/// Lists brewers and chooses the one to act on.
pub struct DeviceService {
    api: Arc<dyn FellowApi>,
}

impl DeviceService {
    /// Creates the service around an injected brewer protocol.
    pub fn new(api: Arc<dyn FellowApi>) -> Self {
        Self { api }
    }

    /// All brewers on the account, flagging the active one.
    pub async fn list(&self) -> Result<Vec<DeviceView>> {
        let devices = self.api.devices().await?;
        let active = self
            .api
            .selected_brewer()
            .or_else(|| match devices.as_slice() {
                [only] => Some(only.id.clone()),
                _ => None,
            });
        Ok(devices
            .into_iter()
            .map(|device| DeviceView {
                active: active.as_deref() == Some(&device.id),
                device,
            })
            .collect())
    }

    /// Finds a brewer by id, else by case-insensitive display name.
    pub async fn resolve(&self, query: &str) -> Result<Device> {
        let devices = self.api.devices().await?;
        if let Some(d) = devices.iter().find(|d| d.id == query) {
            return Ok(d.clone());
        }
        let matches: Vec<&Device> = devices
            .iter()
            .filter(|d| {
                d.display_name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(query))
            })
            .collect();
        match matches.as_slice() {
            [] => Err(CoreError::NotFound(query.into())),
            [one] => Ok((*one).clone()),
            many => Err(CoreError::Ambiguous {
                query: query.into(),
                matches: many.iter().map(|d| d.id.clone()).collect(),
            }),
        }
    }

    /// Makes the matching brewer the active one (not persisted; see [`Aiden::use_brewer`](crate::Aiden::use_brewer)).
    pub async fn select(&self, query: &str) -> Result<Device> {
        let device = self.resolve(query).await?;
        self.api.select_brewer(&device.id);
        Ok(device)
    }

    /// The brewer device-scoped calls act on. Fails when several exist and none is chosen.
    pub async fn active(&self) -> Result<Device> {
        let id = self.api.active_brewer().await?;
        self.api
            .devices()
            .await?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(CoreError::NotFound(id))
    }
}
