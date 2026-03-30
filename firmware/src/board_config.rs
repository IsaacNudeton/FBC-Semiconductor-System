//! Board Configuration — EEPROM defaults + host runtime overrides
//!
//! EEPROM stores production defaults: rail voltage/current limits, ADC calibration
//! offsets, DUT metadata. These are the "factory" values programmed once per BIM.
//!
//! At runtime, the host can override any value without touching the EEPROM.
//! The controller always uses the **effective** config: EEPROM default unless
//! the host has sent an override for that specific field.
//!
//! This is critical for customer flexibility: they change voltages, temperatures,
//! and timing constantly. The EEPROM is the safe baseline; overrides are the
//! experiment. On power cycle, overrides are gone and EEPROM defaults resume.
//!
//! # Override Semantics
//!
//! - Override = 0 or None → use EEPROM default
//! - Override = non-zero → use override value
//! - EEPROM blank (0xFFFF) → use hardcoded safe fallback
//! - No BIM present → all hardcoded safe fallbacks
//!
//! # Safety
//!
//! Rail limits are ALWAYS enforced, even on overrides. The host can raise a
//! voltage limit, but the absolute hardware maximum (from BIM type) is never
//! exceeded. This prevents a software bug from destroying a $50k DUT.

use crate::hal::eeprom::{BimEeprom, RailConfig, BimType, NUM_EEPROM_RAILS};

/// Number of power rails (16 self-describing slots: LCPS J6 + J7 + HCPS)
pub const NUM_RAILS: usize = NUM_EEPROM_RAILS;

/// Number of ADC channels (XADC 0-15 + MAX11131 16-31)
pub const NUM_ADC_CHANNELS: usize = 32;

/// Number of VICOR cores
pub const NUM_VICOR_CORES: usize = 6;

/// Absolute hardware maximums per BIM type (cannot be overridden)
/// These are the physical limits of the power stage — exceeding them damages hardware.
#[derive(Debug, Clone, Copy)]
pub struct HardwareLimits {
    /// Maximum voltage any VICOR core can be set to (mV)
    pub vicor_max_mv: u16,
    /// Maximum current any VICOR core can draw (mA)
    pub vicor_max_current_ma: u16,
    /// Maximum voltage any LCPS rail can be set to (mV)
    pub lcps_max_mv: u16,
    /// Maximum temperature before emergency shutdown (0.1°C units)
    pub temp_shutdown_dc: i16,
}

impl HardwareLimits {
    /// Get hardware limits for a BIM type
    pub const fn for_bim_type(bim_type: BimType) -> Self {
        match bim_type {
            BimType::Normandy => Self {
                vicor_max_mv: 1500,
                vicor_max_current_ma: 5000,
                lcps_max_mv: 5500,
                temp_shutdown_dc: 1500, // 150.0°C
            },
            BimType::SyrosV2 => Self {
                vicor_max_mv: 1200,
                vicor_max_current_ma: 4000,
                lcps_max_mv: 5500,
                temp_shutdown_dc: 1500,
            },
            BimType::Aurora => Self {
                vicor_max_mv: 1500,
                vicor_max_current_ma: 6000,
                lcps_max_mv: 5500,
                temp_shutdown_dc: 1500,
            },
            BimType::Iliad => Self {
                vicor_max_mv: 1500,
                vicor_max_current_ma: 5000,
                lcps_max_mv: 5500,
                temp_shutdown_dc: 1500,
            },
            BimType::Unknown => Self {
                // Conservative defaults for unknown hardware
                vicor_max_mv: 1200,
                vicor_max_current_ma: 3000,
                lcps_max_mv: 3600,
                temp_shutdown_dc: 1250, // 125.0°C
            },
        }
    }
}

/// Runtime rail configuration (one per LCPS rail)
#[derive(Debug, Clone, Copy)]
pub struct EffectiveRail {
    /// Target voltage (mV) — 0 = rail disabled
    pub voltage_mv: u16,
    /// Maximum safe voltage (mV)
    pub max_voltage_mv: u16,
    /// Minimum safe voltage (mV)
    pub min_voltage_mv: u16,
    /// Maximum current (mA)
    pub max_current_ma: u16,
}

impl EffectiveRail {
    pub const fn disabled() -> Self {
        Self {
            voltage_mv: 0,
            max_voltage_mv: 0,
            min_voltage_mv: 0,
            max_current_ma: 0,
        }
    }
}

/// Host override values — None means "use EEPROM default"
#[derive(Clone, Copy)]
pub struct HostOverrides {
    /// Per-rail voltage limit overrides (0 = use EEPROM), indexed by EEPROM rail slot (0-15)
    pub rail_max_voltage_mv: [u16; NUM_RAILS],
    pub rail_min_voltage_mv: [u16; NUM_RAILS],
    pub rail_max_current_ma: [u16; NUM_RAILS],
    /// ADC calibration offset overrides (0 = use EEPROM, i16::MIN = sentinel for "override to 0")
    pub voltage_cal: [i16; 16],
    pub current_cal: [i16; 16],
    /// Whether each calibration channel has been overridden
    pub voltage_cal_set: u16,  // Bitmask: bit N = channel N overridden
    pub current_cal_set: u16,
    /// Temperature setpoint override (0 = use EEPROM/default, in 0.1°C)
    pub temp_setpoint_dc: i16,
    pub temp_setpoint_set: bool,
}

impl HostOverrides {
    pub const fn empty() -> Self {
        Self {
            rail_max_voltage_mv: [0; NUM_RAILS],
            rail_min_voltage_mv: [0; NUM_RAILS],
            rail_max_current_ma: [0; NUM_RAILS],
            voltage_cal: [0; 16],
            current_cal: [0; 16],
            voltage_cal_set: 0,
            current_cal_set: 0,
            temp_setpoint_dc: 0,
            temp_setpoint_set: false,
        }
    }

    /// Clear all overrides (revert to EEPROM defaults)
    pub fn clear(&mut self) {
        *self = Self::empty();
    }
}

/// Board configuration — the single source of truth for runtime parameters
pub struct BoardConfig {
    /// EEPROM data (loaded at boot, immutable during operation)
    eeprom: BimEeprom,
    /// Whether EEPROM was successfully loaded
    eeprom_valid: bool,
    /// Hardware limits for this BIM type
    hw_limits: HardwareLimits,
    /// Host runtime overrides
    overrides: HostOverrides,
}

impl BoardConfig {
    /// Create with no EEPROM (board has no BIM or EEPROM unreadable)
    pub const fn no_eeprom() -> Self {
        Self {
            eeprom: BimEeprom::empty(),
            eeprom_valid: false,
            hw_limits: HardwareLimits::for_bim_type(BimType::Unknown),
            overrides: HostOverrides::empty(),
        }
    }

    /// Create from EEPROM data loaded at boot
    pub fn from_eeprom(eeprom: &BimEeprom) -> Self {
        let bim_type = BimType::from_u8(eeprom.bim_type);
        Self {
            eeprom: *eeprom,
            eeprom_valid: eeprom.is_programmed(),
            hw_limits: HardwareLimits::for_bim_type(bim_type),
            overrides: HostOverrides::empty(),
        }
    }

    /// Get hardware limits (immutable, determined by BIM type)
    pub fn hw_limits(&self) -> &HardwareLimits {
        &self.hw_limits
    }

    /// Get mutable reference to overrides (for host commands)
    pub fn overrides_mut(&mut self) -> &mut HostOverrides {
        &mut self.overrides
    }

    /// Clear all host overrides
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    // =========================================================================
    // Effective Rail Config (EEPROM + overrides merged)
    // =========================================================================

    /// Get effective rail configuration for a given rail index (0-15)
    pub fn effective_rail(&self, rail: usize) -> EffectiveRail {
        if rail >= NUM_RAILS {
            return EffectiveRail::disabled();
        }

        let eeprom_rail = if self.eeprom_valid {
            self.eeprom.rails[rail]
        } else {
            RailConfig::disabled()
        };

        // Skip disabled rail slots
        if !eeprom_rail.is_active() && self.overrides.rail_max_voltage_mv[rail] == 0 {
            return EffectiveRail::disabled();
        }

        // Merge: override wins if non-zero, else EEPROM, else safe default
        let max_v = if self.overrides.rail_max_voltage_mv[rail] != 0 {
            self.overrides.rail_max_voltage_mv[rail]
        } else if eeprom_rail.max_voltage_mv != 0 && eeprom_rail.max_voltage_mv != 0xFFFF {
            eeprom_rail.max_voltage_mv
        } else {
            self.hw_limits.lcps_max_mv // Safe fallback
        };

        let min_v = if self.overrides.rail_min_voltage_mv[rail] != 0 {
            self.overrides.rail_min_voltage_mv[rail]
        } else if eeprom_rail.min_voltage_mv != 0xFFFF {
            eeprom_rail.min_voltage_mv
        } else {
            0
        };

        let max_i = if self.overrides.rail_max_current_ma[rail] != 0 {
            self.overrides.rail_max_current_ma[rail]
        } else if eeprom_rail.max_current_ma != 0 && eeprom_rail.max_current_ma != 0xFFFF {
            eeprom_rail.max_current_ma
        } else {
            self.hw_limits.vicor_max_current_ma
        };

        EffectiveRail {
            voltage_mv: eeprom_rail.nominal_mv(),
            max_voltage_mv: max_v.min(self.hw_limits.lcps_max_mv), // Clamp to hardware max
            min_voltage_mv: min_v,
            max_current_ma: max_i.min(self.hw_limits.vicor_max_current_ma),
        }
    }

    /// Find effective rail config by PMBus channel number (1-24)
    pub fn effective_rail_by_channel(&self, channel: u8) -> Option<(usize, EffectiveRail)> {
        if !self.eeprom_valid {
            return None;
        }
        for i in 0..NUM_RAILS {
            if self.eeprom.rails[i].channel_id == channel {
                let eff = self.effective_rail(i);
                if eff.max_voltage_mv > 0 {
                    return Some((i, eff));
                }
            }
        }
        None
    }

    /// Check if a PMBus voltage command is safe for a given channel
    pub fn check_pmbus_voltage(&self, channel: u8, requested_mv: u16) -> Result<(), RailViolation> {
        match self.effective_rail_by_channel(channel) {
            Some((idx, eff)) => {
                if requested_mv > eff.max_voltage_mv {
                    Err(RailViolation::OverVoltage {
                        rail: idx as u8,
                        requested_mv,
                        limit_mv: eff.max_voltage_mv,
                    })
                } else if requested_mv < eff.min_voltage_mv && requested_mv != 0 {
                    Err(RailViolation::UnderVoltage {
                        rail: idx as u8,
                        requested_mv,
                        limit_mv: eff.min_voltage_mv,
                    })
                } else {
                    Ok(())
                }
            }
            // No EEPROM config for this channel — allow with hardware max clamp
            None => {
                if requested_mv > self.hw_limits.lcps_max_mv {
                    Err(RailViolation::OverVoltage {
                        rail: channel,
                        requested_mv,
                        limit_mv: self.hw_limits.lcps_max_mv,
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    // =========================================================================
    // Voltage Limit Enforcement
    // =========================================================================

    /// Check if a VICOR core voltage is within safe limits
    ///
    /// Returns Ok(clamped_mv) if safe, Err(limit_mv) if the request exceeds
    /// the hardware maximum (which can never be overridden).
    pub fn check_vicor_voltage(&self, core: u8, requested_mv: u16) -> Result<u16, u16> {
        if core == 0 || core > NUM_VICOR_CORES as u8 {
            return Err(0);
        }

        let hw_max = self.hw_limits.vicor_max_mv;

        if requested_mv > hw_max {
            // Absolute hardware limit exceeded — reject
            Err(hw_max)
        } else {
            // Within hardware limits — allow
            // LCPS rail limits apply to LCPS rails, not VICOR cores directly
            Ok(requested_mv)
        }
    }

    /// Check if an LCPS rail voltage is within safe limits
    ///
    /// Returns Ok(()) if within limits, Err with the violated limit.
    pub fn check_rail_voltage(&self, rail: usize, requested_mv: u16) -> Result<(), RailViolation> {
        let eff = self.effective_rail(rail);

        if requested_mv > eff.max_voltage_mv {
            Err(RailViolation::OverVoltage {
                rail: rail as u8,
                requested_mv,
                limit_mv: eff.max_voltage_mv,
            })
        } else if requested_mv < eff.min_voltage_mv && requested_mv != 0 {
            Err(RailViolation::UnderVoltage {
                rail: rail as u8,
                requested_mv,
                limit_mv: eff.min_voltage_mv,
            })
        } else {
            Ok(())
        }
    }

    // =========================================================================
    // ADC Calibration
    // =========================================================================

    /// Get effective voltage calibration offset for an ADC channel (mV)
    pub fn voltage_cal_offset(&self, channel: usize) -> i16 {
        if channel >= 16 {
            return 0;
        }

        // Check if host has overridden this channel
        if self.overrides.voltage_cal_set & (1 << channel) != 0 {
            return self.overrides.voltage_cal[channel];
        }

        // Use EEPROM value if valid
        if self.eeprom_valid {
            self.eeprom.voltage_cal[channel]
        } else {
            0 // No calibration
        }
    }

    /// Get effective current calibration offset for an ADC channel (mA)
    pub fn current_cal_offset(&self, channel: usize) -> i16 {
        if channel >= 16 {
            return 0;
        }

        if self.overrides.current_cal_set & (1 << channel) != 0 {
            return self.overrides.current_cal[channel];
        }

        if self.eeprom_valid {
            self.eeprom.current_cal[channel]
        } else {
            0
        }
    }

    /// Apply voltage calibration to a raw reading
    ///
    /// `raw_mv` is the uncalibrated ADC reading in millivolts.
    /// Returns calibrated value: raw_mv + offset.
    pub fn calibrate_voltage(&self, channel: usize, raw_mv: i32) -> i32 {
        raw_mv + self.voltage_cal_offset(channel) as i32
    }

    /// Apply current calibration to a raw reading
    pub fn calibrate_current(&self, channel: usize, raw_ma: i32) -> i32 {
        raw_ma + self.current_cal_offset(channel) as i32
    }

    // =========================================================================
    // Temperature
    // =========================================================================

    /// Get effective temperature shutdown threshold (0.1°C units)
    pub fn temp_shutdown_dc(&self) -> i16 {
        self.hw_limits.temp_shutdown_dc
    }

    /// Get effective temperature setpoint (0.1°C units, 0 = no setpoint)
    pub fn temp_setpoint_dc(&self) -> i16 {
        if self.overrides.temp_setpoint_set {
            self.overrides.temp_setpoint_dc
        } else if self.eeprom_valid && self.eeprom.thermal.setpoint_dc != 0
            && self.eeprom.thermal.setpoint_dc != 0x7FFF {
            self.eeprom.thermal.setpoint_dc
        } else {
            0 // No setpoint by default
        }
    }

    /// Get project code from EEPROM (e.g., "S0026")
    pub fn project_code(&self) -> &str {
        if self.eeprom_valid {
            self.eeprom.get_project_code()
        } else {
            ""
        }
    }

    /// Get BIM number within the production batch
    pub fn bim_number(&self) -> u16 {
        self.eeprom.bim_number
    }

    // =========================================================================
    // Identity (passthrough from EEPROM)
    // =========================================================================

    /// Whether EEPROM data is valid
    pub fn has_eeprom(&self) -> bool {
        self.eeprom_valid
    }

    /// BIM type
    pub fn bim_type(&self) -> BimType {
        if self.eeprom_valid {
            BimType::from_u8(self.eeprom.bim_type)
        } else {
            BimType::Unknown
        }
    }

    /// Serial number
    pub fn serial(&self) -> u32 {
        self.eeprom.serial_number
    }

    /// Raw EEPROM data (for read-back commands)
    pub fn eeprom(&self) -> &BimEeprom {
        &self.eeprom
    }
}

/// Rail limit violation
#[derive(Debug, Clone, Copy)]
pub enum RailViolation {
    OverVoltage {
        rail: u8,
        requested_mv: u16,
        limit_mv: u16,
    },
    UnderVoltage {
        rail: u8,
        requested_mv: u16,
        limit_mv: u16,
    },
}
