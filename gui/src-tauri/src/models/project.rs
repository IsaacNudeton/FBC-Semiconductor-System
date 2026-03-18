/*
 * models/project.rs — Project entity
 *
 * Ported from: Universal EDA/Models/Project.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub device_id: Uuid,
    pub project_number: String,
    pub system_id: Uuid,
    pub status: ProjectStatus,
    pub cooling: CoolingType,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Planning,
    Active,
    OnHold,
    Complete,
    Cancelled,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        ProjectStatus::Planning
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CoolingType {
    Air,
    Liquid,
    None,
}

impl Default for CoolingType {
    fn default() -> Self {
        CoolingType::Air
    }
}

impl Project {
    pub fn new(device_id: Uuid, project_number: String, system_id: Uuid) -> Self {
        Project {
            id: Uuid::new_v4(),
            device_id,
            project_number,
            system_id,
            status: ProjectStatus::Planning,
            cooling: CoolingType::Air,
            start_date: None,
            end_date: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}
