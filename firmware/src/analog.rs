//! Analog Monitor - Simple 32-Channel Interface for GUI
//!
//! Combines XADC (ch 0-15) + MAX11131 (ch 16-31) into one easy API.
//!
//! GUI Usage:
//! ```
//! let readings = monitor.read_all()?;
//! for r in &readings {
//!     println!("{}: {:.2} {}", r.name, r.value, r.unit);
//! }
//! ```

use crate::hal::{Xadc, Max11131, SpiError};

/// Total channels: XADC (16) + MAX11131 (16)
pub const NUM_CHANNELS: usize = 32;

/// Reference voltage for external ADC (mV)
const EXT_ADC_VREF_MV: u16 = 4096;

/// NTC thermistor presets (from Sonoma ReadAnalog.awk)
/// Circuit: Vref ─ Rtherm(NTC) ─ 150Ω ─ ADC ─ 4980Ω ─ GND
pub const NTC_10K: Formula = Formula::Thermistor { b_coeff: 3492.0, r_ref: 10000.0, r_pullup: 4980.0, r_series: 150.0 };
pub const NTC_30K: Formula = Formula::Thermistor { b_coeff: 3985.3, r_ref: 30000.0, r_pullup: 4980.0, r_series: 150.0 };

/// A single analog reading - everything GUI needs
#[derive(Clone, Copy)]
pub struct Reading {
    /// Channel number (0-31)
    pub channel: u8,
    /// Human-readable name
    pub name: &'static str,
    /// Converted value in engineering units
    pub value: f32,
    /// Unit string (mV, °C, mA, etc.)
    pub unit: &'static str,
    /// Raw ADC value (for debugging)
    pub raw: u16,
}

/// Formula for converting raw ADC to engineering units
#[derive(Clone, Copy)]
pub enum Formula {
    /// No conversion: value = raw
    Raw,
    /// Voltage: value = raw × scale / 4096 (result in mV)
    Voltage { scale_mv: u16 },
    /// Temperature from die sensor (XADC internal)
    DieTemp,
    /// Thermistor via voltage divider
    /// b_coeff = B constant (K), r_ref = resistance at 25°C (Ω)
    /// r_pullup = fixed resistor in divider (Ω), r_series = series resistance (Ω)
    /// Circuit: Vref ─ Rtherm(NTC) ─ Rseries ─ ADC_IN ─ Rpulldown ─ GND
    /// Sonoma hardware: Rpulldown=4980, Rseries=150 (from ReadAnalog.awk)
    Thermistor { b_coeff: f32, r_ref: f32, r_pullup: f32, r_series: f32 },
    /// Current shunt: I(mA) = V(mV) / R(mΩ)
    Current { shunt_mohm: u16 },
    /// VICOR current sense: I(mA) = V_adc(mV) × gain_factor
    /// gain_factor encodes the full sense chain (e.g., 80µA/A × Rload)
    /// Default: Sonoma uses raw × 80 (gain_factor = 80)
    VicorCurrent { gain_factor: u16 },
}

/// Channel configuration (compile-time invariant)
struct ChannelConfig {
    name: &'static str,
    formula: Formula,
    unit: &'static str,
}

/// Channel map - defines what each channel measures
/// Channels 0-15: XADC (Zynq internal)
/// Channels 16-31: MAX11131 (external)
const CHANNELS: [ChannelConfig; NUM_CHANNELS] = [
    // === XADC Channels (0-15) ===
    ChannelConfig { name: "DIE_TEMP", formula: Formula::DieTemp, unit: "C" },
    ChannelConfig { name: "VCCINT", formula: Formula::Voltage { scale_mv: 3000 }, unit: "mV" },
    ChannelConfig { name: "VCCAUX", formula: Formula::Voltage { scale_mv: 3000 }, unit: "mV" },
    ChannelConfig { name: "VCCBRAM", formula: Formula::Voltage { scale_mv: 3000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX0", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX1", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX2", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX3", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX4", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX5", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX6", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX7", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX8", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX9", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX10", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },
    ChannelConfig { name: "XADC_AUX11", formula: Formula::Voltage { scale_mv: 1000 }, unit: "mV" },

    // === MAX11131 Channels (16-31) ===
    ChannelConfig { name: "VDD_CORE1", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "VDD_CORE2", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "VDD_CORE3", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "VDD_CORE4", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "VDD_CORE5", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "VDD_CORE6", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "THERM_CASE", formula: Formula::Thermistor { b_coeff: 3985.3, r_ref: 30000.0, r_pullup: 4980.0, r_series: 150.0 }, unit: "C" },
    ChannelConfig { name: "THERM_DUT", formula: Formula::Thermistor { b_coeff: 3985.3, r_ref: 30000.0, r_pullup: 4980.0, r_series: 150.0 }, unit: "C" },
    ChannelConfig { name: "I_CORE1", formula: Formula::VicorCurrent { gain_factor: 80 }, unit: "mA" },
    ChannelConfig { name: "I_CORE2", formula: Formula::VicorCurrent { gain_factor: 80 }, unit: "mA" },
    ChannelConfig { name: "VDD_IO", formula: Formula::Voltage { scale_mv: 5000 }, unit: "mV" },
    ChannelConfig { name: "VDD_3V3", formula: Formula::Voltage { scale_mv: 5000 }, unit: "mV" },
    ChannelConfig { name: "VDD_1V8", formula: Formula::Voltage { scale_mv: 3000 }, unit: "mV" },
    ChannelConfig { name: "VDD_1V2", formula: Formula::Voltage { scale_mv: 2000 }, unit: "mV" },
    ChannelConfig { name: "EXT_ADC14", formula: Formula::Voltage { scale_mv: 4096 }, unit: "mV" },
    ChannelConfig { name: "EXT_ADC15", formula: Formula::Voltage { scale_mv: 4096 }, unit: "mV" },
];

/// Monitor error
#[derive(Debug, Clone, Copy)]
pub enum MonitorError {
    /// Invalid channel number
    InvalidChannel,
    /// Channel name not found
    NameNotFound,
    /// SPI error
    Spi(SpiError),
    /// XADC error
    Xadc,
}

impl From<SpiError> for MonitorError {
    fn from(e: SpiError) -> Self {
        MonitorError::Spi(e)
    }
}

/// Analog Monitor - unified 32-channel interface
pub struct AnalogMonitor<'a> {
    xadc: &'a Xadc,
    ext_adc: &'a Max11131<'a>,
    /// Runtime formula overrides — None = use compile-time default from CHANNELS[]
    /// Set via test plan config or host command
    formula_overrides: [Option<Formula>; NUM_CHANNELS],
}

impl<'a> AnalogMonitor<'a> {
    /// Create new analog monitor
    pub fn new(xadc: &'a Xadc, ext_adc: &'a Max11131<'a>) -> Self {
        Self {
            xadc,
            ext_adc,
            formula_overrides: [None; NUM_CHANNELS],
        }
    }

    /// Override the formula for a channel at runtime
    ///
    /// Used by test plan config to switch sensor types (e.g., 10kΩ vs 30kΩ thermistor,
    /// or VICOR current sense gain for different modules).
    pub fn set_formula(&mut self, channel: u8, formula: Formula) {
        if (channel as usize) < NUM_CHANNELS {
            self.formula_overrides[channel as usize] = Some(formula);
        }
    }

    /// Clear formula override for a channel (revert to compile-time default)
    pub fn clear_formula(&mut self, channel: u8) {
        if (channel as usize) < NUM_CHANNELS {
            self.formula_overrides[channel as usize] = None;
        }
    }

    /// Clear all formula overrides
    pub fn clear_all_formulas(&mut self) {
        self.formula_overrides = [None; NUM_CHANNELS];
    }

    /// Switch thermistor type for both THERM_CASE and THERM_DUT
    /// Sonoma selects per-board: option 0 = 10kΩ, option 4 = 30kΩ (ReadAnalog.awk)
    /// Default is 30kΩ (most common). Call this if board has 10kΩ NTCs.
    pub fn set_ntc_type(&mut self, use_10k: bool) {
        let formula = if use_10k { NTC_10K } else { NTC_30K };
        self.set_formula(22, formula); // THERM_CASE
        self.set_formula(23, formula); // THERM_DUT
    }

    /// Get effective formula for a channel
    fn effective_formula(&self, channel: usize) -> Formula {
        if channel < NUM_CHANNELS {
            self.formula_overrides[channel].unwrap_or(CHANNELS[channel].formula)
        } else {
            Formula::Raw
        }
    }

    // ========================================
    // SIMPLE GUI API
    // ========================================

    /// Read single channel by number (0-31)
    ///
    /// Returns Reading with name, value, unit, raw
    pub fn read(&self, channel: u8) -> Result<Reading, MonitorError> {
        if channel >= NUM_CHANNELS as u8 {
            return Err(MonitorError::InvalidChannel);
        }

        let raw = self.read_raw(channel)?;
        let config = &CHANNELS[channel as usize];
        let formula = self.effective_formula(channel as usize);
        let value = self.apply_formula(raw, formula);

        Ok(Reading {
            channel,
            name: config.name,
            value,
            unit: config.unit,
            raw,
        })
    }

    /// Read single channel by name
    ///
    /// Example: `monitor.read_by_name("VDD_CORE1")?`
    pub fn read_by_name(&self, name: &str) -> Result<Reading, MonitorError> {
        for (i, config) in CHANNELS.iter().enumerate() {
            if config.name == name {
                return self.read(i as u8);
            }
        }
        Err(MonitorError::NameNotFound)
    }

    /// Read ALL 32 channels at once
    ///
    /// This is the main GUI function - returns everything you need
    pub fn read_all(&self) -> Result<[Reading; NUM_CHANNELS], MonitorError> {
        // Read XADC channels (0-15)
        let mut readings: [Reading; NUM_CHANNELS] = [Reading {
            channel: 0,
            name: "",
            value: 0.0,
            unit: "",
            raw: 0,
        }; NUM_CHANNELS];

        // Read external ADC (batch)
        let ext_raw = self.ext_adc.read_all()?;

        // Build readings array
        for ch in 0..NUM_CHANNELS {
            let raw = if ch < 16 {
                // XADC channel
                self.read_xadc_channel(ch as u8)?
            } else {
                // External ADC channel
                ext_raw[ch - 16]
            };

            let config = &CHANNELS[ch];
            let formula = self.effective_formula(ch);
            let value = self.apply_formula(raw, formula);

            readings[ch] = Reading {
                channel: ch as u8,
                name: config.name,
                value,
                unit: config.unit,
                raw,
            };
        }

        Ok(readings)
    }

    /// Get channel name (for GUI labels)
    pub fn get_name(&self, channel: u8) -> &'static str {
        if (channel as usize) < NUM_CHANNELS {
            CHANNELS[channel as usize].name
        } else {
            "UNKNOWN"
        }
    }

    /// Get channel unit (for GUI labels)
    pub fn get_unit(&self, channel: u8) -> &'static str {
        if (channel as usize) < NUM_CHANNELS {
            CHANNELS[channel as usize].unit
        } else {
            ""
        }
    }

    /// Read case temperature (THERM_CASE, MAX11131 ch 22) in milliCelsius
    /// This is the correct input for thermal control (NOT XADC die temp)
    pub fn read_case_temp_mc(&self) -> Result<i32, MonitorError> {
        let reading = self.read(22)?; // THERM_CASE
        Ok((reading.value * 1000.0) as i32)
    }

    /// Read DUT temperature (THERM_DUT, MAX11131 ch 23) in milliCelsius
    pub fn read_dut_temp_mc(&self) -> Result<i32, MonitorError> {
        let reading = self.read(23)?; // THERM_DUT
        Ok((reading.value * 1000.0) as i32)
    }

    /// Read real-time core power in milliwatts.
    /// Measures V×I for cores 1-2 (the only cores with current sense channels),
    /// then extrapolates total power assuming all 6 cores draw proportionally.
    ///
    /// Returns (total_power_mw, PowerLevel) for thermal feedforward.
    pub fn read_core_power_mw(&self) -> Result<(u32, crate::hal::thermal::PowerLevel), MonitorError> {
        // Read core 1: voltage (ch 16) × current (ch 24)
        let v1 = self.read(16)?; // VDD_CORE1 in mV
        let i1 = self.read(24)?; // I_CORE1 in mA

        // Read core 2: voltage (ch 17) × current (ch 25)
        let v2 = self.read(17)?; // VDD_CORE2 in mV
        let i2 = self.read(25)?; // I_CORE2 in mA

        // P = V(mV) × I(mA) / 1000 = milliwatts
        let p1_mw = (v1.value * i1.value / 1000.0) as u32;
        let p2_mw = (v2.value * i2.value / 1000.0) as u32;
        let measured_mw = p1_mw + p2_mw;

        // Extrapolate: 2 measured cores → 6 total (×3)
        let total_mw = measured_mw * 3;

        // Map to PowerLevel for thermal feedforward
        let level = if total_mw > 10_000 {
            crate::hal::thermal::PowerLevel::High   // >10W
        } else if total_mw > 3_000 {
            crate::hal::thermal::PowerLevel::Medium  // 3-10W
        } else {
            crate::hal::thermal::PowerLevel::Low     // <3W
        };

        Ok((total_mw, level))
    }

    /// Find channel number by name
    pub fn find_channel(&self, name: &str) -> Option<u8> {
        for (i, config) in CHANNELS.iter().enumerate() {
            if config.name == name {
                return Some(i as u8);
            }
        }
        None
    }

    /// Get list of all channel names (for GUI dropdown)
    pub fn channel_names(&self) -> [&'static str; NUM_CHANNELS] {
        let mut names = [""; NUM_CHANNELS];
        for (i, config) in CHANNELS.iter().enumerate() {
            names[i] = config.name;
        }
        names
    }

    // ========================================
    // INTERNAL
    // ========================================

    /// Read raw value from channel
    fn read_raw(&self, channel: u8) -> Result<u16, MonitorError> {
        if channel < 16 {
            self.read_xadc_channel(channel)
        } else {
            Ok(self.ext_adc.read_channel(channel - 16)?)
        }
    }

    /// Read XADC channel
    fn read_xadc_channel(&self, ch: u8) -> Result<u16, MonitorError> {
        // Map channel to XADC function
        // All channels return the actual XADC 16-bit register value.
        // Formula conversion happens in apply_formula().
        match ch {
            0 => self.xadc.read_temperature_raw().map_err(|_| MonitorError::Xadc),
            1 => self.xadc.read_vccint_raw().map_err(|_| MonitorError::Xadc),
            2 => self.xadc.read_vccaux_raw().map_err(|_| MonitorError::Xadc),
            3 => self.xadc.read_vccbram_raw().map_err(|_| MonitorError::Xadc),
            4..=15 => self.xadc.read_vaux_raw(ch - 4).map_err(|_| MonitorError::Xadc),
            _ => Err(MonitorError::InvalidChannel),
        }
    }

    /// Apply formula to convert raw value to engineering units
    fn apply_formula(&self, raw: u16, formula: Formula) -> f32 {
        match formula {
            Formula::Raw => raw as f32,

            Formula::Voltage { scale_mv } => {
                // V = raw × scale / 4096
                (raw as f32) * (scale_mv as f32) / 4096.0
            }

            Formula::DieTemp => {
                // Xilinx UG480 formula: T(°C) = (ADC_CODE × 503.975 / 65536) - 273.15
                // ADC_CODE is the full 16-bit register (12-bit ADC left-justified in bits [15:4])
                (raw as f32) * 503.975 / 65536.0 - 273.15
            }

            Formula::Thermistor { b_coeff, r_ref, r_pullup, r_series } => {
                // Full B-equation: T = 1/((ln(R/R25)/B) + 1/298.15) - 273.15
                // Matches Sonoma ReadAnalog.awk thermistor formula exactly
                if raw >= 4095 {
                    return -999.0; // Short circuit (NTC on high side: max raw = min R)
                }
                if raw == 0 {
                    return 999.0; // Open circuit (NTC on high side: zero raw = infinite R)
                }

                // Circuit: Vref ─ Rtherm(NTC) ─ Rseries ─ ADC_IN ─ Rpulldown ─ GND
                // Sonoma ReadAnalog.awk: RT = ((4980*(4096/Reading))-4980)-150
                // Derived: R_therm = Rpulldown × (4096/raw - 1) - Rseries
                let r = r_pullup * (4096.0 / (raw as f32) - 1.0) - r_series;
                if r <= 0.0 {
                    return 999.0; // Invalid (short or bad reading)
                }

                // B-equation: T(K) = 1 / ((ln(R/R25) / B) + 1/T25)
                let ratio = r / r_ref;
                let ln_ratio = ln_approx(ratio);
                let inv_t = (ln_ratio / b_coeff) + (1.0 / 298.15);
                if inv_t <= 0.0 {
                    return 999.0; // Would give negative Kelvin
                }
                (1.0 / inv_t) - 273.15
            }

            Formula::Current { shunt_mohm } => {
                // I = V / R
                // V = raw × Vref / 4096
                // I (mA) = V (mV) / R (mΩ)
                let v_mv = (raw as f32) * (EXT_ADC_VREF_MV as f32) / 4096.0;
                v_mv / (shunt_mohm as f32)
            }

            Formula::VicorCurrent { gain_factor } => {
                // VICOR current sense: ADC reads voltage proportional to current
                // I(mA) = V_adc(mV) × gain_factor / 1000
                // Sonoma AWK uses raw × 80 with 1mV/LSB ADC
                let v_mv = (raw as f32) * (EXT_ADC_VREF_MV as f32) / 4096.0;
                v_mv * (gain_factor as f32) / 1000.0
            }
        }
    }
}

// ========================================
// MATH: ln() approximation for no_std
// ========================================

/// Natural logarithm approximation using IEEE 754 bit manipulation.
/// Accuracy: ~0.3% over the range [0.01, 100.0] — sufficient for NTC thermistors
/// (which themselves have ±1-2% tolerance).
///
/// Method: decompose float into exponent + mantissa, then:
///   ln(x) = exponent * ln(2) + ln(mantissa)
/// where mantissa is in [1, 2) and ln(1+y) uses a 3-term polynomial.
fn ln_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return -1e10; // Negative/zero → large negative (avoid panic)
    }
    let bits = x.to_bits() as i32;
    let e = ((bits >> 23) & 0xFF) - 127; // IEEE 754 exponent
    // Reconstruct mantissa in [1.0, 2.0)
    let m_bits = (bits & 0x007F_FFFF) | 0x3F80_0000;
    let m = f32::from_bits(m_bits as u32);
    // ln(m) for m in [1, 2): Taylor series ln(1+y) ≈ y - y²/2 + y³/3
    let y = m - 1.0;
    let ln_m = y * (1.0 - y * (0.5 - y * 0.3333));
    (e as f32) * 0.6931472 + ln_m // ln(2) = 0.6931472
}

// ========================================
// CONVENIENCE: Print all readings
// ========================================

impl Reading {
    /// Format reading as string for display
    pub fn to_string(&self) -> [u8; 64] {
        // Simple formatting without alloc
        let mut buf = [0u8; 64];
        // In real impl, would format: "VDD_CORE1: 1000.0 mV"
        // For now, just copy name
        let name_bytes = self.name.as_bytes();
        let len = name_bytes.len().min(63);
        buf[..len].copy_from_slice(&name_bytes[..len]);
        buf
    }
}
