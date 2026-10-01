use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A brewer registered on the account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Brewer id used in every device-scoped path.
    pub id: String,
    /// User-chosen name, if any.
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    /// All other fields of the device config, preserved verbatim.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Device {
    /// `Name (id)` or just the id when unnamed.
    pub fn label(&self) -> String {
        match &self.display_name {
            Some(name) if !name.is_empty() => format!("{name} ({})", self.id),
            _ => self.id.clone(),
        }
    }
}
