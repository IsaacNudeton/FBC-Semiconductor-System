/*
 * models/board.rs — Board entity
 *
 * Ported from: Universal EDA/Models/Board.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: Uuid,
    pub lot_id: Uuid,
    pub system_id: Uuid,
    pub position: BoardPosition,
    pub status: BoardStatus,
    pub serial: String,
    pub device_id: Uuid,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardPosition {
    pub shelf: i32,
    pub tray: i32,
    pub slot: i32,
    pub position_label: String,  // "FRONT", "REAR"
    pub slot_label: String,       // "A", "B"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BoardStatus {
    Empty,
    Loaded,
    Testing,
    Complete,
    Failed,
    Shutdown,
}

impl Default for BoardStatus {
    fn default() -> Self {
        BoardStatus::Empty
    }
}

impl Board {
    pub fn new(lot_id: Uuid, system_id: Uuid, position: BoardPosition, serial: String, device_id: Uuid) -> Self {
        Board {
            id: Uuid::new_v4(),
            lot_id,
            system_id,
            position,
            status: BoardStatus::Empty,
            serial,
            device_id,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}
