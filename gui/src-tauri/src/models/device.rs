/*
 * models/device.rs — Device entity
 *
 * Ported from: Universal EDA/Models/Device.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub customer: String,
    pub device_name: String,
    pub device_number: String,
    pub device_family: String,
    pub package_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Device {
    pub fn new(customer: String, device_name: String, device_number: String, device_family: String, package_type: String) -> Self {
        Device {
            id: Uuid::new_v4(),
            customer,
            device_name,
            device_number,
            device_family,
            package_type,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}
