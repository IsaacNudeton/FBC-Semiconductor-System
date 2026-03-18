/*
 * models/position.rs — Board position helpers
 *
 * Ported from: Universal EDA/Models/PositionIdentifier.cs
 */

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionIdentifier {
    pub shelf: i32,
    pub chamber: i32,
    pub tray: i32,
    pub slot: i32,
    pub position_label: String,  // "FRONT", "REAR"
    pub slot_label: String,       // "A", "B"
}

impl PositionIdentifier {
    pub fn new(shelf: i32, chamber: i32, tray: i32, slot: i32, position_label: &str, slot_label: &str) -> Self {
        PositionIdentifier {
            shelf,
            chamber,
            tray,
            slot,
            position_label: position_label.to_string(),
            slot_label: slot_label.to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        format!(
            "S{}-C{}-T{}-{}{}",
            self.shelf, self.chamber, self.tray, self.position_label, self.slot_label
        )
    }

    pub fn from_string(s: &str) -> Option<Self> {
        // Parse format: "S1-C1-T1-FRONTA"
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let shelf = parts[0].strip_prefix('S')?.parse().ok()?;
        let chamber = parts[1].strip_prefix('C')?.parse().ok()?;
        let tray = parts[2].strip_prefix('T')?.parse().ok()?;

        let pos = parts[3];
        let (position_label, slot_label) = if pos.ends_with('A') || pos.ends_with('B') {
            (&pos[..pos.len()-1], &pos[pos.len()-1..])
        } else {
            (pos, "")
        };

        Some(PositionIdentifier {
            shelf,
            chamber,
            tray,
            slot: 0,
            position_label: position_label.to_string(),
            slot_label: slot_label.to_string(),
        })
    }
}
