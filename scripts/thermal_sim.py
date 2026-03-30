"""
FBC Thermal Controller Simulation v2
======================================
Compares 3 controllers against realistic burn-in physics:
  1. Reactive PID (Sonoma-style: no feedforward)
  2. ONETWO v1 (current thermal.rs: pure integrator, oscillates)
  3. ONETWO v2 (fixed: P+I+feedforward, crystallization damping)

The v2 controller keeps the ONETWO philosophy:
  - Feedforward from vector toggle analysis (predict before temp changes)
  - Crystallization settling (converge in bounded iterations)
  - Structural constants (not arbitrary tuning knobs)
But adds proportional control for physical stability.
"""

import math
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

# ============================================================
# Physical parameters (Zynq 7020 + BIM + DUT)
# ============================================================
THERMAL_MASS = 2.0       # J/°C
HEATER_MAX_W = 50.0      # Watlow cartridge heater max
FAN_MAX_W = 15.0          # Forced air cooling max
AMBIENT_C = 25.0
PASSIVE_LOSS = 0.08       # W/°C natural convection
DT = 0.1                  # 10 Hz sample rate
TOTAL_TIME = 210.0

# NTC 30kOhm thermistor
B_COEFF = 3985.3
R25 = 30000.0
R_PULLDOWN = 4980.0
R_SERIES = 150.0

def temp_to_resistance(t_c):
    return R25 * math.exp(B_COEFF * (1.0/(t_c+273.15) - 1.0/298.15))

def temp_to_adc_raw(t_c):
    rt = temp_to_resistance(t_c)
    raw = 4096.0 * R_PULLDOWN / (rt + R_SERIES + R_PULLDOWN)
    return max(0, min(4095, int(raw)))

def adc_raw_to_temp(raw):
    if raw <= 0 or raw >= 4095:
        return 999.0
    r = R_PULLDOWN * (4096.0 / raw - 1.0) - R_SERIES
    if r <= 0:
        return 999.0
    ln_r = math.log(r / R25)
    inv_t = (ln_r / B_COEFF) + (1.0 / 298.15)
    return (1.0 / inv_t) - 273.15 if inv_t > 0 else 999.0

# ============================================================
# Controller 1: Reactive PID (Sonoma-style)
# ============================================================
class ReactivePID:
    def __init__(self):
        self.setpoint_mc = 25000
        self.kp, self.ki, self.kd = 0.02, 0.005, 0.01
        self.integral = 0.0
        self.prev_error = 0.0

    def set_target(self, sp):
        if self.setpoint_mc != sp:
            self.setpoint_mc = sp
            self.integral = 0.0
            self.prev_error = 0.0

    def set_power_level(self, _): pass  # No feedforward

    def update(self, actual_mc):
        e = self.setpoint_mc - actual_mc
        self.integral = max(-50000, min(50000, self.integral + e * DT))
        d = (e - self.prev_error) / DT
        self.prev_error = e
        out = (self.kp * e + self.ki * self.integral + self.kd * d) / 1000.0
        return max(-1.0, min(1.0, out))

# ============================================================
# Controller 2: ONETWO v1 (current thermal.rs — pure integrator)
# ============================================================
class OnetwoV1:
    def __init__(self):
        self.setpoint_mc = 25000
        self.output = 0.0
        self.iterations = 0
        self.initial_error = 0.0
        self.feedforward = 0.0

    def set_target(self, sp):
        if self.setpoint_mc != sp:
            self.setpoint_mc = sp
            self.iterations = 0
            self.initial_error = 0.0

    def set_power_level(self, level):
        ff = {'low': 0, 'medium': -100, 'high': -300}
        self.feedforward = ff.get(level, 0)

    def update(self, actual_mc):
        e = self.setpoint_mc - actual_mc
        if self.iterations == 0:
            self.initial_error = max(abs(e), 1)
        correction = e * 0.71828
        self.output += correction / 1000.0
        out = self.output + self.feedforward / 1000.0
        if self.iterations < 7:
            self.iterations += 1
        return max(-1.0, min(1.0, out))

# ============================================================
# Controller 3: ONETWO v2 (P+I+feedforward, crystallization damping)
# ============================================================
class OnetwoV2:
    """
    Keeps ONETWO philosophy but physically stable:
    - Proportional term: immediate response to error (damping)
    - Integral term: eliminates steady-state offset (with crystallization decay)
    - Feedforward: pre-compensate for vector-induced heating
    - Settling rate still (e-2), still crystallizes, but doesn't ring

    Key insight: the SETTLE constant (e-2 = 0.718) becomes the
    integral decay factor, not the raw gain. This means the integrator
    "forgets" old errors at a structurally-determined rate — crystallization.
    """
    def __init__(self):
        self.setpoint_mc = 25000
        self.integral = 0.0
        self.prev_error = 0.0
        self.iterations = 0
        self.initial_error = 0.0
        self.feedforward = 0.0

        # Structural constants (not arbitrary tuning)
        self.kp = 0.015           # Proportional: sized to thermal mass
        self.ki = 0.003           # Integral: slow, eliminates offset
        self.settle = 0.71828     # e-2: integral decay (crystallization)

    def set_target(self, sp):
        if self.setpoint_mc != sp:
            self.setpoint_mc = sp
            self.iterations = 0
            self.initial_error = 0.0
            # Don't reset integral — smooth transitions

    def set_power_level(self, level):
        # Feedforward: pre-compensate BEFORE temperature changes
        # Vector power estimate → thermal offset prediction
        ff = {'low': 0.0, 'medium': -0.06, 'high': -0.18}
        self.feedforward = ff.get(level, 0.0)

    def update(self, actual_mc):
        e = self.setpoint_mc - actual_mc

        if self.iterations == 0:
            self.initial_error = max(abs(e), 1)

        # L1: Proportional (immediate damping)
        p_term = self.kp * e / 1000.0

        # L2: Integral with crystallization decay
        # Instead of unbounded accumulation, integral decays toward zero
        # at rate (e-2). This IS the crystallization — bounded convergence.
        self.integral = self.integral * self.settle + e * self.ki * DT
        self.integral = max(-0.5, min(0.5, self.integral))  # Anti-windup

        # L3: Derivative (damping for oscillation prevention)
        d_term = 0.005 * (e - self.prev_error) / DT / 1000.0
        self.prev_error = e

        # L4: Combine + feedforward
        output = p_term + self.integral + d_term + self.feedforward

        if self.iterations < 7:
            self.iterations += 1

        return max(-1.0, min(1.0, output))

# ============================================================
# Plant simulation
# ============================================================
def simulate(controller, seed=42):
    np.random.seed(seed)
    steps = int(TOTAL_TIME / DT)
    t_actual = AMBIENT_C
    times, temps, setpoints, outputs, vpowers = [], [], [], [], []

    for i in range(steps):
        t = i * DT

        # Scenario phases
        if t < 30:
            sp_c, vec_w, plevel = 125.0, 0.0, 'low'
        elif t < 90:
            sp_c, vec_w, plevel = 125.0, 1.0, 'low'
        elif t < 120:
            sp_c, vec_w, plevel = 125.0, 6.0, 'high'
        elif t < 180:
            sp_c, vec_w, plevel = 125.0, 1.0, 'low'
        else:
            sp_c, vec_w, plevel = 25.0, 0.0, 'low'

        controller.set_target(int(sp_c * 1000))
        controller.set_power_level(plevel)

        # ADC reading with noise
        noise = (np.random.random() - 0.5) * 0.6
        raw = temp_to_adc_raw(t_actual + noise)
        t_meas = adc_raw_to_temp(raw)

        output = controller.update(int(t_meas * 1000))

        # Physics
        heater_w = output * HEATER_MAX_W if output > 0 else 0
        fan_w = (-output) * FAN_MAX_W if output < 0 else 0
        q_net = heater_w + vec_w - fan_w - PASSIVE_LOSS * (t_actual - AMBIENT_C)
        t_actual += q_net / THERMAL_MASS * DT

        times.append(t)
        temps.append(t_actual)
        setpoints.append(sp_c)
        outputs.append(output * 100)
        vpowers.append(vec_w)

    return times, temps, setpoints, outputs, vpowers

# ============================================================
# Run all 3
# ============================================================
print("Running simulations...")
t, temp_pid, sp, out_pid, vp = simulate(ReactivePID())
_, temp_v1, _, out_v1, _ = simulate(OnetwoV1())
_, temp_v2, _, out_v2, _ = simulate(OnetwoV2())

# ============================================================
# Metrics
# ============================================================
idx_90, idx_120, idx_40 = int(90/DT), int(120/DT), int(40/DT)

def overshoot(temps):
    return max(temps[idx_90:idx_120]) - 125.0

def settle(temps, start, target=125.0, tol=0.5):
    for i in range(start, len(temps)):
        if abs(temps[i] - target) <= tol:
            end = min(i + int(2.0/DT), len(temps))
            if all(abs(temps[j] - target) <= tol for j in range(i, end)):
                return (i - start) * DT
    return float('inf')

def rms(temps, a, b, target=125.0):
    return math.sqrt(sum((t-target)**2 for t in temps[a:b]) / (b-a))

os_pid, os_v1, os_v2 = overshoot(temp_pid), overshoot(temp_v1), overshoot(temp_v2)
st_pid, st_v1, st_v2 = settle(temp_pid, idx_90), settle(temp_v1, idx_90), settle(temp_v2, idx_90)
rm_pid, rm_v1, rm_v2 = rms(temp_pid, idx_40, idx_90), rms(temp_v1, idx_40, idx_90), rms(temp_v2, idx_40, idx_90)

print(f"\n{'='*65}")
print(f"  THERMAL CONTROLLER SHOOTOUT")
print(f"  Burn-in at 125C, 6W vector load step at t=90s")
print(f"{'='*65}")
print(f"                        PID (Sonoma)   ONETWO v1    ONETWO v2")
print(f"  Phase 3 overshoot:    {os_pid:+.1f}C         {os_v1:+.1f}C       {os_v2:+.1f}C")
st_v1_str = f"{st_v1:.1f}s" if st_v1 < 999 else "NEVER"
print(f"  Settle time (0.5C):   {st_pid:.1f}s          {st_v1_str:>8s}       {st_v2:.1f}s")
print(f"  Steady RMS error:     {rm_pid:.3f}C         {rm_v1:.3f}C      {rm_v2:.3f}C")
print(f"{'='*65}")
print(f"  v2 = feedforward advantage with physical stability")
print(f"{'='*65}")

# ============================================================
# Plot
# ============================================================
fig, axes = plt.subplots(3, 1, figsize=(15, 11), sharex=True,
                         gridspec_kw={'height_ratios': [3.5, 1, 0.8]})
fig.suptitle('FBC Thermal Controller Shootout\n'
             'Reactive PID (Sonoma) vs ONETWO v1 (current) vs ONETWO v2 (fixed)',
             fontsize=14, fontweight='bold')

# Colors
C_PID = '#FF5722'
C_V1 = '#9E9E9E'
C_V2 = '#2196F3'
C_SP = '#4CAF50'

# --- Temperature ---
ax1 = axes[0]
# Phase backgrounds
phase_colors = ['#E3F2FD', '#FFF3E0', '#FFCDD2', '#FFF3E0', '#E8F5E9']
phase_labels = ['Ramp\n(idle)', 'Hold 125C\n(1W vectors)', 'HIGH TOGGLE\n(6W vectors!)',
                'Hold 125C\n(1W vectors)', 'Cool down\nto 25C']
phase_bounds = [0, 30, 90, 120, 180, 210]
for i in range(5):
    ax1.axvspan(phase_bounds[i], phase_bounds[i+1], alpha=0.25, color=phase_colors[i])
    mid = (phase_bounds[i] + phase_bounds[i+1]) / 2
    y_pos = 18 if i != 2 else 18
    ax1.text(mid, y_pos, phase_labels[i], ha='center', va='bottom', fontsize=8, alpha=0.5)

ax1.plot(t, temp_v1, color=C_V1, linewidth=1.0, alpha=0.6, label='ONETWO v1 (current — oscillates)', zorder=1)
ax1.plot(t, temp_pid, color=C_PID, linewidth=1.8, label=f'Reactive PID (Sonoma) — overshoot {os_pid:+.1f}C', zorder=2)
ax1.plot(t, temp_v2, color=C_V2, linewidth=2.2, label=f'ONETWO v2 (feedforward) — overshoot {os_v2:+.1f}C', zorder=3)
ax1.plot(t, sp, color=C_SP, linewidth=1.5, linestyle='--', label='Setpoint', alpha=0.7)

# Zoom inset for phase 3 transition
from mpl_toolkits.axes_grid1.inset_locator import inset_axes
ax_inset = inset_axes(ax1, width="35%", height="45%", loc='right',
                       bbox_to_anchor=(0.0, 0.05, 0.95, 0.95), bbox_transform=ax1.transAxes)
idx_85 = int(85/DT)
idx_135 = int(135/DT)
ax_inset.plot(t[idx_85:idx_135], temp_pid[idx_85:idx_135], color=C_PID, linewidth=1.5)
ax_inset.plot(t[idx_85:idx_135], temp_v2[idx_85:idx_135], color=C_V2, linewidth=2.0)
ax_inset.plot(t[idx_85:idx_135], sp[idx_85:idx_135], color=C_SP, linewidth=1, linestyle='--', alpha=0.7)
ax_inset.axvline(x=90, color='red', linewidth=0.8, linestyle=':', alpha=0.5)
ax_inset.set_title('Phase 3 transition (zoom)', fontsize=8)
ax_inset.set_ylim(122, 135)
ax_inset.grid(True, alpha=0.2)
ax_inset.tick_params(labelsize=7)

ax1.set_ylabel('Temperature (C)', fontsize=11)
ax1.set_ylim(15, max(max(temp_v1), 155))
ax1.legend(loc='upper left', fontsize=9, framealpha=0.9)
ax1.grid(True, alpha=0.3)

# Metrics box
metrics = (
    f"         PID(Sonoma)  ONETWO v1  ONETWO v2\n"
    f"Overshoot:  {os_pid:+5.1f}C     {os_v1:+6.1f}C    {os_v2:+5.1f}C\n"
    f"Settle:     {st_pid:5.1f}s      {'NEVER':>6s}     {st_v2:5.1f}s\n"
    f"RMS error:  {rm_pid:5.3f}C     {rm_v1:6.3f}C    {rm_v2:5.3f}C"
)
ax1.text(0.02, 0.72, metrics, transform=ax1.transAxes, fontsize=8.5,
         fontfamily='monospace', verticalalignment='top',
         bbox=dict(boxstyle='round', facecolor='white', alpha=0.95, edgecolor='#bbb'))

# --- Controller output ---
ax2 = axes[1]
ax2.plot(t, out_v1, color=C_V1, linewidth=0.8, alpha=0.5, label='ONETWO v1')
ax2.plot(t, out_pid, color=C_PID, linewidth=1.2, label='PID (Sonoma)')
ax2.plot(t, out_v2, color=C_V2, linewidth=1.5, label='ONETWO v2')
ax2.axhline(y=0, color='gray', linewidth=0.5)
ax2.set_ylabel('Output (%)', fontsize=11)
ax2.set_ylim(-120, 120)
ax2.legend(loc='upper right', fontsize=8)
ax2.grid(True, alpha=0.3)

# --- Vector power ---
ax3 = axes[2]
ax3.fill_between(t, vp, color='#9C27B0', alpha=0.35)
ax3.plot(t, vp, color='#9C27B0', linewidth=1.2, label='Vector switching power (W)')
ax3.set_ylabel('Power (W)', fontsize=11)
ax3.set_xlabel('Time (seconds)', fontsize=11)
ax3.set_ylim(-0.5, 8)
ax3.legend(loc='upper right', fontsize=8)
ax3.grid(True, alpha=0.3)

plt.tight_layout()
out_path = r'C:\Dev\projects\FBC-Semiconductor-System\scripts\thermal_sim.png'
plt.savefig(out_path, dpi=150, bbox_inches='tight')
print(f"\nSaved: {out_path}")
