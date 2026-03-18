/*
 * models/controller.rs — Controller entity
 *
 * Ported from: Universal EDA/Models/Controller.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Controller {
    pub id: Uuid,
    pub board_id: Option<Uuid>,
    pub ip_address: String,
    pub mac_address: String,
    pub status: ControllerStatus,
    pub firmware_version: String,
    pub last_seen: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerStatus {
    Online,
    Offline,
    Testing,
    Error,
}

impl Default for ControllerStatus {
    fn default() -> Self {
        ControllerStatus::Offline
    }
}

impl Controller {
    pub fn new(ip_address: String, mac_address: String) -> Self {
        Controller {
            id: Uuid::new_v4(),
            board_id: None,
            ip_address,
            mac_address,
            status: ControllerStatus::Offline,
            firmware_version: String::new(),
            last_seen: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn mark_online(&mut self) {
        self.status = ControllerStatus::Online;
        self.last_seen = Some(chrono::Utc::now().timestamp_millis());
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    pub fn mark_offline(&mut self) {
        self.status = ControllerStatus::Offline;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}
