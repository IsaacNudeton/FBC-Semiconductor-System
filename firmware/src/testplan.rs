//! Test Plan Execution Engine
//!
//! Autonomous burn-in execution: upload vectors once, define a step sequence,
//! firmware runs it for hours/days without PC involvement.
//!
//! # Execution Model
//!
//! ```text
//! Steps:  [0: continuity] [1: init] [2: stress_a] [3: stress_b]
//!                                     ^--- loop_start
//!
//! First pass:  step 0 → step 1 → step 2 → step 3
//! Loop passes: step 2 → step 3 → step 2 → step 3 → ...
//! Until:       total_duration_secs elapsed, or host STOP, or Abort step fails
//! ```
//!
//! # Step Types
//!
//! Each step references a DDR slot (pre-uploaded .fbc file) and defines:
//! - How long to run (0 = single pass, >0 = repeat for N seconds)
//! - What to do on error (Abort = stop plan, Continue = log and move on)
//!
//! # Per-Step Results
//!
//! After each step completes, firmware records:
//! - Pass/fail status
//! - Error count
//! - Cycles executed
//! - Time spent
//!
//! These are reported via heartbeat (step context) and GET_PLAN_STATUS.

use core::ptr::{read_volatile, write_volatile};
use crate::ddr_slots::MAX_SLOTS;

// =============================================================================
// DDR Persistence
// =============================================================================

/// DDR address for plan checkpoint (after slot table, 4KB gap)
const PLAN_CHECKPOINT_BASE: usize = 0x0030_1000;

/// Magic for checkpoint validation
const CHECKPOINT_MAGIC: u32 = 0x504C_414E; // "PLAN"

// =============================================================================
// Test Plan Structures
// =============================================================================

/// Maximum number of steps in a test plan (real projects: up to 91 steps)
pub const MAX_STEPS: usize = 96;

/// What to do when a step has vector errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FailAction {
    /// Stop the entire plan. Board goes to Done state.
    Abort = 0,
    /// Log the error, proceed to next step.
    Continue = 1,
}

impl FailAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => FailAction::Continue,
            _ => FailAction::Abort,
        }
    }
}

/// Sentinel: no temperature change for this step
pub const TEMP_NO_CHANGE: i16 = 0x7FFF;
/// Sentinel: no clock change for this step
pub const CLOCK_NO_CHANGE: u8 = 0xFF;

/// A single step in the test plan
#[derive(Debug, Clone, Copy)]
pub struct TestStep {
    /// Pattern index on SD card (0-255, references PatternDirectory)
    pub pattern_id: u8,
    /// How long to run this step in seconds.
    /// 0 = run vectors once to completion (single pass).
    /// >0 = loop vectors repeatedly for this many seconds.
    pub duration_secs: u32,
    /// What to do if vectors produce errors
    pub fail_action: FailAction,
    /// Max allowed errors before fail_action triggers (0 = any error triggers)
    pub error_threshold: u32,
    /// Temperature setpoint in deci-Celsius (0.1°C units).
    /// 0x7FFF = no change (keep previous). Applied before vectors start.
    pub temp_setpoint_dc: i16,
    /// Clock divider (0=5MHz, 1=10MHz, 2=25MHz, 3=50MHz, 4=100MHz).
    /// 0xFF = no change. Applied before vectors start.
    pub clock_div: u8,
}

impl TestStep {
    pub const fn empty() -> Self {
        Self {
            pattern_id: 0,
            duration_secs: 0,
            fail_action: FailAction::Abort,
            error_threshold: 0,
            temp_setpoint_dc: TEMP_NO_CHANGE,
            clock_div: CLOCK_NO_CHANGE,
        }
    }
}

/// Complete test plan
#[derive(Clone, Copy)]
pub struct TestPlan {
    /// Number of active steps (1-8)
    pub num_steps: u8,
    /// On loop-back, start from this step index (0 = replay everything).
    /// First pass always runs 0..num_steps. Subsequent loops run loop_start..num_steps.
    pub loop_start: u8,
    /// Total plan duration in seconds (0 = single pass, no looping).
    /// When >0, the plan loops from loop_start until this time elapses.
    pub total_duration_secs: u32,
    /// The steps
    pub steps: [TestStep; MAX_STEPS],
}

impl TestPlan {
    pub const fn empty() -> Self {
        Self {
            num_steps: 0,
            loop_start: 0,
            total_duration_secs: 0,
            steps: [TestStep::empty(); MAX_STEPS],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.num_steps > 0
            && (self.num_steps as usize) <= MAX_STEPS
            && (self.loop_start as usize) < self.num_steps as usize
    }

    /// Parse test plan from protocol payload.
    ///
    /// Format:
    ///   [0]      num_steps
    ///   [1]      loop_start
    ///   [2..6]   total_duration_secs (BE)
    ///   Per step (13 bytes each):
    ///     [0]      pattern_id
    ///     [1..5]   duration_secs (BE)
    ///     [5]      fail_action (0=Abort, 1=Continue)
    ///     [6..10]  error_threshold (BE)
    ///     [10..12] temp_setpoint_dc (BE i16, 0x7FFF = no change)
    ///     [12]     clock_div (0-4, 0xFF = no change)
    pub fn from_payload(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        let num_steps = data[0];
        let loop_start = data[1];
        let total_duration_secs = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);

        if num_steps == 0 || num_steps as usize > MAX_STEPS {
            return None;
        }
        if loop_start >= num_steps {
            return None;
        }

        let step_data = &data[6..];
        if step_data.len() < num_steps as usize * 13 {
            return None;
        }

        let mut plan = Self {
            num_steps,
            loop_start,
            total_duration_secs,
            steps: [TestStep::empty(); MAX_STEPS],
        };

        for i in 0..num_steps as usize {
            let off = i * 13;
            plan.steps[i] = TestStep {
                pattern_id: step_data[off],
                duration_secs: u32::from_be_bytes([
                    step_data[off + 1],
                    step_data[off + 2],
                    step_data[off + 3],
                    step_data[off + 4],
                ]),
                fail_action: FailAction::from_u8(step_data[off + 5]),
                error_threshold: u32::from_be_bytes([
                    step_data[off + 6],
                    step_data[off + 7],
                    step_data[off + 8],
                    step_data[off + 9],
                ]),
                temp_setpoint_dc: i16::from_be_bytes([
                    step_data[off + 10],
                    step_data[off + 11],
                ]),
                clock_div: step_data[off + 12],
            };

            // Validate slot reference
            if plan.steps[i].pattern_id as usize >= MAX_SLOTS {
                return None;
            }
        }

        Some(plan)
    }
}

// =============================================================================
// Step Result
// =============================================================================

/// Result of a single step execution
#[derive(Debug, Clone, Copy)]
pub struct StepResult {
    /// Step index
    pub step_index: u8,
    /// 0 = pass, 1 = fail (errors > threshold), 2 = aborted
    pub status: u8,
    /// Total errors accumulated across all loops of this step
    pub total_errors: u32,
    /// Number of complete loop iterations
    pub loops_completed: u32,
    /// Time spent on this step (seconds)
    pub elapsed_secs: u32,
}

impl StepResult {
    pub const fn empty() -> Self {
        Self {
            step_index: 0,
            status: 0,
            total_errors: 0,
            loops_completed: 0,
            elapsed_secs: 0,
        }
    }
}

// =============================================================================
// Plan Execution State Machine
// =============================================================================

/// Current state of plan execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanState {
    /// No plan loaded or plan complete
    Idle,
    /// Vectors being DMA'd to FPGA for current step
    Loading,
    /// Vectors running on FPGA
    Running,
    /// Current step's vectors finished, deciding next action
    StepDone,
    /// Entire plan completed successfully
    Complete,
    /// Plan aborted due to step failure
    Aborted,
}

/// Test plan execution engine
pub struct PlanExecutor {
    /// The plan being executed
    plan: TestPlan,
    /// Current execution state
    pub state: PlanState,
    /// Current step index
    pub current_step: u8,
    /// Current loop iteration within the step
    pub current_loop: u32,
    /// Whether we're in the first pass (run all steps) or looping (loop_start..N)
    first_pass: bool,
    /// Wall-clock time when plan started (ms since boot)
    plan_start_ms: u32,
    /// Wall-clock time when current step started (ms since boot)
    step_start_ms: u32,
    /// Accumulated errors for current step
    step_errors: u32,
    /// Results per step
    pub results: [StepResult; MAX_STEPS],
    /// Total plan loops completed
    pub plan_loops: u32,
}

impl PlanExecutor {
    pub const fn new() -> Self {
        Self {
            plan: TestPlan::empty(),
            state: PlanState::Idle,
            current_step: 0,
            current_loop: 0,
            first_pass: true,
            plan_start_ms: 0,
            step_start_ms: 0,
            step_errors: 0,
            results: [StepResult::empty(); MAX_STEPS],
            plan_loops: 0,
        }
    }

    /// Load a test plan. Does not start execution.
    pub fn set_plan(&mut self, plan: TestPlan) {
        self.plan = plan;
        self.state = PlanState::Idle;
        self.current_step = 0;
        self.current_loop = 0;
        self.first_pass = true;
        self.step_errors = 0;
        self.plan_loops = 0;
        for r in self.results.iter_mut() {
            *r = StepResult::empty();
        }
    }

    /// Start executing the loaded plan.
    /// Returns the pattern_id of the first step to load.
    pub fn start(&mut self, now_ms: u32) -> Option<u8> {
        if !self.plan.is_valid() {
            return None;
        }
        self.state = PlanState::Loading;
        self.current_step = 0;
        self.current_loop = 0;
        self.first_pass = true;
        self.plan_start_ms = now_ms;
        self.step_start_ms = now_ms;
        self.step_errors = 0;
        self.plan_loops = 0;
        for r in self.results.iter_mut() {
            *r = StepResult::empty();
        }
        Some(self.plan.steps[0].pattern_id)
    }

    /// Called when FPGA reports vectors are running (DMA complete, decoder enabled).
    pub fn on_running(&mut self) {
        if self.state == PlanState::Loading {
            self.state = PlanState::Running;
        }
    }

    /// Called when FPGA decoder hits HALT (one pass of vectors complete).
    /// Returns the action the main loop should take.
    pub fn on_vectors_done(&mut self, error_count: u32, now_ms: u32) -> PlanAction {
        if self.state != PlanState::Running {
            return PlanAction::None;
        }

        self.step_errors += error_count;
        self.current_loop += 1;

        let step = &self.plan.steps[self.current_step as usize];

        // Check if step duration has elapsed (if time-limited)
        let step_elapsed_ms = now_ms.wrapping_sub(self.step_start_ms);
        let step_time_up = step.duration_secs > 0
            && step_elapsed_ms >= step.duration_secs * 1000;

        // Check error threshold
        let errors_exceeded = step.error_threshold > 0
            && self.step_errors > step.error_threshold;
        let any_error_aborts = step.error_threshold == 0 && self.step_errors > 0;
        let should_fail = errors_exceeded || any_error_aborts;

        if should_fail && step.fail_action == FailAction::Abort {
            // Record result and abort
            self.record_step_result(2, now_ms); // status=2 aborted
            self.state = PlanState::Aborted;
            return PlanAction::PlanAborted;
        }

        if should_fail && step.fail_action == FailAction::Continue {
            // Record as failed but move on
            self.record_step_result(1, now_ms); // status=1 fail
            return self.advance_step(now_ms);
        }

        // No failure. Time-limited step still going?
        if step.duration_secs > 0 && !step_time_up {
            // Re-DMA same slot, keep looping
            self.state = PlanState::Loading;
            return PlanAction::LoadPattern(step.pattern_id);
        }

        // Single-pass step (duration=0) or time expired
        if step.duration_secs == 0 || step_time_up {
            self.record_step_result(0, now_ms); // status=0 pass
            return self.advance_step(now_ms);
        }

        PlanAction::None
    }

    /// Advance to next step. Returns action for main loop.
    fn advance_step(&mut self, now_ms: u32) -> PlanAction {
        let next = self.current_step + 1;

        if next >= self.plan.num_steps {
            // Reached end of steps. Check if we should loop.
            let plan_elapsed_ms = now_ms.wrapping_sub(self.plan_start_ms);
            let plan_time_up = self.plan.total_duration_secs > 0
                && plan_elapsed_ms >= self.plan.total_duration_secs * 1000;

            if self.plan.total_duration_secs == 0 || plan_time_up {
                // Single pass or time exceeded — done
                self.state = PlanState::Complete;
                return PlanAction::PlanComplete;
            }

            // Loop back
            self.first_pass = false;
            self.plan_loops += 1;
            self.current_step = self.plan.loop_start;
            self.current_loop = 0;
            self.step_errors = 0;
            self.step_start_ms = now_ms;
            self.state = PlanState::Loading;
            return PlanAction::LoadPattern(self.plan.steps[self.current_step as usize].pattern_id);
        }

        // Next step
        self.current_step = next;
        self.current_loop = 0;
        self.step_errors = 0;
        self.step_start_ms = now_ms;
        self.state = PlanState::Loading;
        PlanAction::LoadPattern(self.plan.steps[next as usize].pattern_id)
    }

    /// Record result for current step
    fn record_step_result(&mut self, status: u8, now_ms: u32) {
        let idx = self.current_step as usize;
        self.results[idx] = StepResult {
            step_index: self.current_step,
            status,
            total_errors: self.step_errors,
            loops_completed: self.current_loop,
            elapsed_secs: now_ms.wrapping_sub(self.step_start_ms) / 1000,
        };
    }

    /// Stop execution (from host STOP command or emergency)
    pub fn stop(&mut self, now_ms: u32) {
        if self.state == PlanState::Running || self.state == PlanState::Loading {
            self.record_step_result(2, now_ms);
            self.state = PlanState::Idle;
        }
    }

    /// Get reference to current plan
    pub fn plan(&self) -> &TestPlan {
        &self.plan
    }

    /// Check if a plan is loaded and ready
    pub fn has_plan(&self) -> bool {
        self.plan.is_valid()
    }

    /// Get current step's config (temp + clock) for main loop to apply
    pub fn current_step(&self) -> &TestStep {
        &self.plan.steps[self.current_step as usize]
    }

    /// Serialize plan status for GET_PLAN_STATUS response.
    ///
    /// Format:
    ///   [0]      state (PlanState as u8)
    ///   [1]      current_step
    ///   [2..6]   current_loop (BE)
    ///   [6..10]  plan_loops (BE)
    ///   [10..14] elapsed_secs (BE) — total plan time
    ///   [14..18] step_errors (BE) — current step errors
    ///   Per completed step (9 bytes each):
    ///     [0]      step_index
    ///     [1]      status
    ///     [2..6]   total_errors (BE)
    ///     [6..10]  loops_completed (BE) -- wait that's 9 not right
    /// Actually let's keep it simpler:
    ///   [14+i*8 .. 14+(i+1)*8] per step:
    ///     [0]    status
    ///     [1..5] total_errors (BE)
    ///     [5..8] loops_completed (u24 BE, enough for 16M loops)
    pub fn serialize_status(&self, buf: &mut [u8], now_ms: u32) -> usize {
        if buf.len() < 14 {
            return 0;
        }

        buf[0] = self.state as u8;
        buf[1] = self.current_step;
        buf[2..6].copy_from_slice(&self.current_loop.to_be_bytes());
        buf[6..10].copy_from_slice(&self.plan_loops.to_be_bytes());

        let elapsed = now_ms.wrapping_sub(self.plan_start_ms) / 1000;
        buf[10..14].copy_from_slice(&elapsed.to_be_bytes());

        let mut pos = 14;
        for i in 0..self.plan.num_steps as usize {
            if pos + 8 > buf.len() {
                break;
            }
            let r = &self.results[i];
            buf[pos] = r.status;
            buf[pos + 1..pos + 5].copy_from_slice(&r.total_errors.to_be_bytes());
            let loops_24 = r.loops_completed.min(0xFF_FFFF);
            buf[pos + 5] = ((loops_24 >> 16) & 0xFF) as u8;
            buf[pos + 6] = ((loops_24 >> 8) & 0xFF) as u8;
            buf[pos + 7] = (loops_24 & 0xFF) as u8;
            pos += 8;
        }
        pos
    }

    // =========================================================================
    // DDR Checkpoint Persistence
    // =========================================================================

    /// Write current plan state to DDR for warm-reset survival.
    /// Call periodically (~10s) and on state transitions.
    ///
    /// Layout at PLAN_CHECKPOINT_BASE (64 bytes):
    ///   [0:4]   magic = 0x504C414E ("PLAN")
    ///   [4]     state (PlanState as u8)
    ///   [5]     current_step
    ///   [6]     num_steps
    ///   [7]     loop_start
    ///   [8:12]  total_duration_secs
    ///   [12:16] elapsed_secs (total plan elapsed at checkpoint)
    ///   [16:20] plan_loops
    ///   [20:24] step_errors
    ///   [24:28] current_loop
    ///   [28:32] bim_serial (for invalidation on BIM swap)
    ///   [32:36] step_start_elapsed_secs (time into plan when current step started)
    ///   [36:64] reserved
    pub fn checkpoint_to_ddr(&self, now_ms: u32, bim_serial: u32) {
        // Only persist if plan is active
        if self.state == PlanState::Idle {
            return;
        }

        let elapsed_secs = now_ms.wrapping_sub(self.plan_start_ms) / 1000;
        let step_start_elapsed = self.step_start_ms.wrapping_sub(self.plan_start_ms) / 1000;
        let ptr = PLAN_CHECKPOINT_BASE as *mut u32;

        unsafe {
            write_volatile(ptr, CHECKPOINT_MAGIC);
            write_volatile(ptr.add(1),
                (self.state as u32)
                | ((self.current_step as u32) << 8)
                | ((self.plan.num_steps as u32) << 16)
                | ((self.plan.loop_start as u32) << 24)
            );
            write_volatile(ptr.add(2), self.plan.total_duration_secs);
            write_volatile(ptr.add(3), elapsed_secs);
            write_volatile(ptr.add(4), self.plan_loops);
            write_volatile(ptr.add(5), self.step_errors);
            write_volatile(ptr.add(6), self.current_loop);
            write_volatile(ptr.add(7), bim_serial);
            write_volatile(ptr.add(8), step_start_elapsed);
        }
    }

    /// Try to restore plan state from DDR after warm reset.
    /// Returns true if a valid checkpoint was found and the plan should resume.
    /// The caller must verify bim_serial matches before calling resume_from_ddr().
    pub fn read_checkpoint_from_ddr(&self) -> Option<PlanCheckpoint> {
        let ptr = PLAN_CHECKPOINT_BASE as *const u32;
        unsafe {
            let magic = read_volatile(ptr);
            if magic != CHECKPOINT_MAGIC {
                return None;
            }
            let word1 = read_volatile(ptr.add(1));
            let state_u8 = (word1 & 0xFF) as u8;

            // Only resume if plan was actually running
            let state = match state_u8 {
                1 | 2 => {}, // Loading or Running — resume
                _ => return None, // Idle/Complete/Aborted — nothing to resume
            };
            let _ = state;

            Some(PlanCheckpoint {
                state: state_u8,
                current_step: ((word1 >> 8) & 0xFF) as u8,
                num_steps: ((word1 >> 16) & 0xFF) as u8,
                loop_start: ((word1 >> 24) & 0xFF) as u8,
                total_duration_secs: read_volatile(ptr.add(2)),
                elapsed_secs: read_volatile(ptr.add(3)),
                plan_loops: read_volatile(ptr.add(4)),
                step_errors: read_volatile(ptr.add(5)),
                current_loop: read_volatile(ptr.add(6)),
                bim_serial: read_volatile(ptr.add(7)),
                step_start_elapsed_secs: read_volatile(ptr.add(8)),
            })
        }
    }

    /// Resume from a DDR checkpoint. Adjusts internal timers so elapsed time
    /// is preserved. The plan definition must already be set via set_plan().
    ///
    /// `now_ms` = current time. The executor pretends it started
    /// `elapsed_secs` ago, so remaining time is calculated correctly.
    pub fn resume_from_checkpoint(&mut self, cp: &PlanCheckpoint, now_ms: u32) {
        // Reconstruct timing: pretend we started elapsed_secs ago
        let fake_start_ms = now_ms.wrapping_sub(cp.elapsed_secs * 1000);
        let fake_step_start_ms = now_ms.wrapping_sub(
            (cp.elapsed_secs - cp.step_start_elapsed_secs) * 1000
        );

        self.state = PlanState::Loading; // will re-DMA current step's slot
        self.current_step = cp.current_step;
        self.current_loop = cp.current_loop;
        self.first_pass = cp.current_step < cp.loop_start || cp.plan_loops == 0;
        self.plan_start_ms = fake_start_ms;
        self.step_start_ms = fake_step_start_ms;
        self.step_errors = cp.step_errors;
        self.plan_loops = cp.plan_loops;
    }

    /// Clear the DDR checkpoint (call when plan completes or is explicitly stopped).
    pub fn clear_checkpoint(&self) {
        unsafe {
            write_volatile(PLAN_CHECKPOINT_BASE as *mut u32, 0);
        }
    }
}

// =============================================================================
// Checkpoint Data (read from DDR)
// =============================================================================

/// Plan state snapshot read from DDR after warm reset
#[derive(Debug, Clone, Copy)]
pub struct PlanCheckpoint {
    pub state: u8,
    pub current_step: u8,
    pub num_steps: u8,
    pub loop_start: u8,
    pub total_duration_secs: u32,
    pub elapsed_secs: u32,
    pub plan_loops: u32,
    pub step_errors: u32,
    pub current_loop: u32,
    pub bim_serial: u32,
    pub step_start_elapsed_secs: u32,
}

// =============================================================================
// Actions
// =============================================================================

/// Action the main loop should take after plan state change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanAction {
    /// Nothing to do
    None,
    /// Load vectors from this DDR slot and DMA to FPGA
    LoadPattern(u8),
    /// Plan completed successfully
    PlanComplete,
    /// Plan aborted due to step failure
    PlanAborted,
}
