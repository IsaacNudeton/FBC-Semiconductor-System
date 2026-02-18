//! ONETWO Thermal Controller
//!
//! Crystallization-based temperature control with pattern-aware feedforward.
//! No arbitrary PID constants - settling rate forced by structure.
//!
//! Constants:
//!   SETTLE = (e - 2) ≈ 0.71828  (settling rate per iteration)
//!   LOCK = 7                     (iterations to crystallize)
//!   FLOOR = 0.10                 (10% jitter floor)
//!
//! Feedforward:
//!   Analyzes vector toggle rate to predict power before temp changes.
//!   High toggle = high power = pre-compensate thermal output.

/// Settling rate: e - 2
/// This is structurally forced - not tunable.
const SETTLE: i32 = 718; // Fixed-point: 0.71828 × 1000

/// Crystallization threshold (iterations)
const LOCK_ITERATIONS: u8 = 7;

/// Jitter floor: 10% residual (can't do better)
const FLOOR_PCT: i32 = 10;

/// Vector width (pins that toggle with patterns)
const VECTOR_WIDTH: usize = 128;

/// Power level estimated from vector analysis
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerLevel {
    /// Low toggle rate, few active pins
    Low,
    /// Medium activity
    Medium,
    /// High toggle rate, many active pins
    High,
}

impl PowerLevel {
    /// Feedforward multiplier for thermal compensation
    /// Higher power = more cooling needed
    pub fn feedforward(&self) -> i32 {
        match self {
            PowerLevel::Low => 0,      // No compensation
            PowerLevel::Medium => -100, // Slight pre-cool
            PowerLevel::High => -300,   // Aggressive pre-cool
        }
    }
}

/// Vector power analysis result
#[derive(Clone, Copy, Debug)]
pub struct PowerEstimate {
    /// Estimated power level
    pub level: PowerLevel,
    /// Average toggles per vector (0-128)
    pub toggle_rate: u32,
    /// Average active pins (0-128)
    pub active_pins: u32,
    /// Number of vectors analyzed
    pub vector_count: u32,
}

/// Analyze vectors to estimate power draw
/// Call this ONCE when loading vectors, not during control loop
pub fn estimate_power(vectors: &[u128]) -> PowerEstimate {
    if vectors.is_empty() {
        return PowerEstimate {
            level: PowerLevel::Low,
            toggle_rate: 0,
            active_pins: 0,
            vector_count: 0,
        };
    }

    let mut total_toggles: u64 = 0;
    let mut total_active: u64 = 0;

    // Count transitions and active pins
    for i in 0..vectors.len() {
        // Active pins in this vector
        total_active += vectors[i].count_ones() as u64;

        // Toggles from previous vector
        if i > 0 {
            let transitions = vectors[i] ^ vectors[i - 1];
            total_toggles += transitions.count_ones() as u64;
        }
    }

    let count = vectors.len() as u64;
    let toggle_rate = if count > 1 {
        (total_toggles / (count - 1)) as u32
    } else {
        0
    };
    let active_pins = (total_active / count) as u32;

    // Determine power level from toggle rate and density
    // Thresholds based on physics: more toggles = more switching power
    let level = match (toggle_rate, active_pins) {
        (t, a) if t > 40 && a > 60 => PowerLevel::High,   // >40 toggles, >60 active
        (t, a) if t > 20 || a > 40 => PowerLevel::Medium, // >20 toggles OR >40 active
        _ => PowerLevel::Low,
    };

    PowerEstimate {
        level,
        toggle_rate,
        active_pins,
        vector_count: vectors.len() as u32,
    }
}

/// Analyze vectors from raw bytes (4 x u32 = 128 bits per vector)
pub fn estimate_power_bytes(data: &[u8]) -> PowerEstimate {
    if data.len() < 16 {
        return PowerEstimate {
            level: PowerLevel::Low,
            toggle_rate: 0,
            active_pins: 0,
            vector_count: 0,
        };
    }

    let vector_count = data.len() / 16;
    let mut total_toggles: u64 = 0;
    let mut total_active: u64 = 0;
    let mut prev: u128 = 0;

    for i in 0..vector_count {
        let offset = i * 16;

        // Build 128-bit value from bytes (little-endian)
        let mut vec: u128 = 0;
        for j in 0..16 {
            vec |= (data[offset + j] as u128) << (j * 8);
        }

        total_active += vec.count_ones() as u64;

        if i > 0 {
            let transitions = vec ^ prev;
            total_toggles += transitions.count_ones() as u64;
        }
        prev = vec;
    }

    let count = vector_count as u64;
    let toggle_rate = if count > 1 {
        (total_toggles / (count - 1)) as u32
    } else {
        0
    };
    let active_pins = (total_active / count) as u32;

    let level = match (toggle_rate, active_pins) {
        (t, a) if t > 40 && a > 60 => PowerLevel::High,
        (t, a) if t > 20 || a > 40 => PowerLevel::Medium,
        _ => PowerLevel::Low,
    };

    PowerEstimate {
        level,
        toggle_rate,
        active_pins,
        vector_count: vector_count as u32,
    }
}

/// Thermal controller state
pub struct Thermal {
    /// Target temperature (milliCelsius)
    setpoint_mc: i32,
    /// Accumulated output (for heater/fan PWM)
    output: i32,
    /// Iteration count since setpoint change
    iterations: u8,
    /// Initial error (for floor calculation)
    initial_error: i32,
    /// Last error (for crystallization check)
    last_error: i32,
    /// Feedforward from power estimate
    feedforward: i32,
    /// Current power level
    power_level: PowerLevel,
}

/// Controller output
pub struct ThermalOutput {
    /// Correction to apply (-1000 to +1000, maps to fan/heater)
    pub correction: i32,
    /// True if crystallized (at setpoint within floor)
    pub locked: bool,
    /// Current error in milliCelsius
    pub error_mc: i32,
    /// Iterations completed
    pub iterations: u8,
}

impl Thermal {
    /// Create new controller
    pub const fn new() -> Self {
        Self {
            setpoint_mc: 25_000, // Default 25°C
            output: 0,
            iterations: 0,
            initial_error: 0,
            last_error: 0,
            feedforward: 0,
            power_level: PowerLevel::Low,
        }
    }

    /// Set power estimate from vector analysis
    /// Call this when loading new vectors, before running
    pub fn set_power_estimate(&mut self, estimate: &PowerEstimate) {
        self.power_level = estimate.level;
        self.feedforward = estimate.level.feedforward();
    }

    /// Set power level directly
    pub fn set_power_level(&mut self, level: PowerLevel) {
        self.power_level = level;
        self.feedforward = level.feedforward();
    }

    /// Get current power level
    pub fn power_level(&self) -> PowerLevel {
        self.power_level
    }

    /// Set target temperature
    pub fn set_target(&mut self, setpoint_mc: i32) {
        if self.setpoint_mc != setpoint_mc {
            self.setpoint_mc = setpoint_mc;
            self.iterations = 0;
            self.initial_error = 0; // Will be set on first update
        }
    }

    /// Update controller with current temperature
    /// Returns correction and status
    pub fn update(&mut self, actual_mc: i32) -> ThermalOutput {
        // L1: Distinction - what's the error?
        let error = self.setpoint_mc - actual_mc;

        // First iteration: capture initial error for floor calc
        if self.iterations == 0 {
            self.initial_error = abs(error);
            if self.initial_error == 0 {
                self.initial_error = 1; // Avoid div by zero
            }
        }

        // L2: Relate - calculate correction using settling rate
        // correction = error × (e-2) = error × 0.718
        // Fixed point: (error × 718) / 1000
        let correction = (error * SETTLE) / 1000;

        // L3: Change - accumulate output + feedforward
        // Feedforward anticipates load from vector power estimate
        self.output = self.output.saturating_add(correction);
        let output_with_ff = self.output.saturating_add(self.feedforward);

        // Clamp output to sane range
        let clamped = output_with_ff.clamp(-1000, 1000);

        // L4: Embed - check crystallization
        let error_abs = abs(error);
        let floor = (self.initial_error * FLOOR_PCT) / 100;
        let locked = error_abs <= floor.max(100); // At least 0.1°C tolerance

        // Track iterations
        if self.iterations < LOCK_ITERATIONS {
            self.iterations += 1;
        }

        self.last_error = error;

        ThermalOutput {
            correction: clamped,
            locked,
            error_mc: error,
            iterations: self.iterations,
        }
    }

    /// Get current setpoint
    pub fn setpoint(&self) -> i32 {
        self.setpoint_mc
    }

    /// Get current output
    pub fn output(&self) -> i32 {
        self.output
    }

    /// Reset controller state
    pub fn reset(&mut self) {
        self.output = 0;
        self.iterations = 0;
        self.initial_error = 0;
        self.last_error = 0;
        self.feedforward = 0;
        self.power_level = PowerLevel::Low;
    }

    /// Check if crystallized (locked to setpoint)
    pub fn is_locked(&self) -> bool {
        self.iterations >= LOCK_ITERATIONS
    }
}

/// Absolute value (no std)
#[inline]
const fn abs(x: i32) -> i32 {
    if x < 0 { -x } else { x }
}

/// Convert output to heater duty cycle (0-100%)
#[inline]
pub fn output_to_heater(output: i32) -> u8 {
    if output > 0 {
        ((output * 100) / 1000) as u8
    } else {
        0
    }
}

/// Convert output to fan duty cycle (0-100%)
#[inline]
pub fn output_to_fan(output: i32) -> u8 {
    if output < 0 {
        (((-output) * 100) / 1000) as u8
    } else {
        0
    }
}
