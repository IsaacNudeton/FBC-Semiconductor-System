/*
 * models/lot.rs — LOT entity
 *
 * Ported from: Universal EDA/Models/LOT.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    pub id: Uuid,
    pub project_id: Uuid,
    pub system_id: Uuid,
    pub lot_number: String,
    pub customer_lot: String,
    pub step: LotStep,
    pub status: LotStatus,
    pub expected_qty: i32,
    pub running_qty: i32,
    pub good: i32,
    pub reject: i32,
    pub missing: i32,
    pub received_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LotStep {
    Received,
    Setup,
    Loading,
    BurnIn,
    Readpoint,
    Unloading,
    Shipping,
    Complete,
}

impl Default for LotStep {
    fn default() -> Self {
        LotStep::Received
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LotStatus {
    Active,
    OnHold,
    Complete,
    Cancelled,
}

impl Default for LotStatus {
    fn default() -> Self {
        LotStatus::Active
    }
}

impl Lot {
    pub fn new(project_id: Uuid, system_id: Uuid, lot_number: String, customer_lot: String, expected_qty: i32) -> Self {
        Lot {
            id: Uuid::new_v4(),
            project_id,
            system_id,
            lot_number,
            customer_lot,
            step: LotStep::Received,
            status: LotStatus::Active,
            expected_qty,
            running_qty: expected_qty,
            good: 0,
            reject: 0,
            missing: 0,
            received_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn advance_step(&mut self) {
        self.step = match self.step {
            LotStep::Received => LotStep::Setup,
            LotStep::Setup => LotStep::Loading,
            LotStep::Loading => LotStep::BurnIn,
            LotStep::BurnIn => LotStep::Readpoint,
            LotStep::Readpoint => LotStep::Unloading,
            LotStep::Unloading => LotStep::Shipping,
            LotStep::Shipping => LotStep::Complete,
            LotStep::Complete => LotStep::Complete,
        };

        if self.step == LotStep::BurnIn && self.started_at.is_none() {
            self.started_at = Some(chrono::Utc::now().timestamp_millis());
        }

        if self.step == LotStep::Complete {
            self.completed_at = Some(chrono::Utc::now().timestamp_millis());
        }

        self.updated_at();
    }

    fn updated_at(&mut self) {
        // In real implementation, update updated_at timestamp
    }
}
