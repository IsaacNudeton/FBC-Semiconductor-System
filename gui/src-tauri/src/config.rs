//! Rack Configuration
//!
//! Defines the physical layout of the burn-in system.

use serde::{Deserialize, Serialize};

/// Rack configuration - defines the physical layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RackConfig {
    /// Number of shelves in the rack (default: 11)
    pub shelves: u8,

    /// Boards per tray (default: 4)
    pub boards_per_tray: u8,

    /// Whether rack has front and back trays (default: true)
    pub dual_tray: bool,

    /// Board slot assignments (MAC -> position)
    pub assignments: Vec<BoardAssignment>,
}

impl Default for RackConfig {
    fn default() -> Self {
        Self {
            shelves: 11,
            boards_per_tray: 4,
            dual_tray: true,
            assignments: Vec::new(),
        }
    }
}

impl RackConfig {
    /// Total board capacity
    pub fn capacity(&self) -> u32 {
        let trays_per_shelf = if self.dual_tray { 2 } else { 1 };
        (self.shelves as u32) * trays_per_shelf * (self.boards_per_tray as u32)
    }

    /// Get position for a MAC address
    pub fn get_position(&self, mac: &str) -> Option<SlotPosition> {
        self.assignments
            .iter()
            .find(|a| a.mac == mac)
            .map(|a| a.position.clone())
    }

    /// Assign a board to a position
    pub fn assign(&mut self, mac: String, position: SlotPosition) {
        // Remove existing assignment for this MAC
        self.assignments.retain(|a| a.mac != mac);
        // Remove existing assignment for this position
        self.assignments.retain(|a| a.position != position);
        // Add new assignment
        self.assignments.push(BoardAssignment { mac, position });
    }

    /// Auto-assign boards to available slots
    pub fn auto_assign(&mut self, macs: &[String]) {
        let mut slot_idx = 0;
        let trays_per_shelf = if self.dual_tray { 2 } else { 1 };
        let total_slots = (self.shelves as usize) * trays_per_shelf * (self.boards_per_tray as usize);

        for mac in macs {
            if self.get_position(mac).is_some() {
                continue; // Already assigned
            }

            // Find next available slot
            while slot_idx < total_slots {
                let shelf = (slot_idx / (trays_per_shelf * self.boards_per_tray as usize)) as u8 + 1;
                let tray_idx = (slot_idx / self.boards_per_tray as usize) % trays_per_shelf;
                let tray = if tray_idx == 0 { TrayPosition::Front } else { TrayPosition::Back };
                let slot = (slot_idx % self.boards_per_tray as usize) as u8 + 1;

                let position = SlotPosition { shelf, tray, slot };

                // Check if this position is taken
                if !self.assignments.iter().any(|a| a.position == position) {
                    self.assign(mac.clone(), position);
                    slot_idx += 1;
                    break;
                }

                slot_idx += 1;
            }
        }
    }
}

/// Board assignment to a physical slot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardAssignment {
    pub mac: String,
    pub position: SlotPosition,
}

/// Physical position in the rack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotPosition {
    /// Shelf number (1-based)
    pub shelf: u8,
    /// Front or back tray
    pub tray: TrayPosition,
    /// Slot on the tray (1-based)
    pub slot: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrayPosition {
    Front,
    Back,
}

impl std::fmt::Display for SlotPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tray = match self.tray {
            TrayPosition::Front => "F",
            TrayPosition::Back => "B",
        };
        write!(f, "{}-{}{}", self.shelf, tray, self.slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_capacity() {
        let config = RackConfig::default();
        // 11 shelves * 2 trays * 4 boards = 88
        assert_eq!(config.capacity(), 88);
    }

    #[test]
    fn test_auto_assign() {
        let mut config = RackConfig::default();
        let macs: Vec<String> = (0..10)
            .map(|i| format!("00:0A:35:00:00:{:02X}", i))
            .collect();

        config.auto_assign(&macs);

        assert_eq!(config.assignments.len(), 10);
    }
}
