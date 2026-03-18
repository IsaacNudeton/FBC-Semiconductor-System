/*
 * models/user.rs — User entity
 *
 * Ported from: Universal EDA/Models/User.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: UserRole,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Operator,
    Tech,
    Engineer,
    Admin,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Operator
    }
}

impl User {
    pub fn new(username: String, display_name: String, role: UserRole) -> Self {
        User {
            id: Uuid::new_v4(),
            username,
            display_name,
            role,
            active: true,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}
