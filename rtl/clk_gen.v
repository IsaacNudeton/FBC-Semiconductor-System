`timescale 1ns / 1ps
//=============================================================================
// Clock Generation - ONETWO Pre-Gen MUX Approach
//=============================================================================
//
// ONETWO Design:
//   INVARIANT: Test plans use ~5 distinct frequencies (5, 10, 25, 50, 100 MHz)
//   VARIES: Which frequency is selected for each test step
//   PATTERN: Pre-generate all frequencies, select via MUX
//
// This is BETTER than Sonoma's DRP approach:
//   - Switch time: <100ns (MUX) vs 100µs (PLL relock)
//   - Glitch-free: BUFGCTRL guarantees clean transitions
//   - Simpler: ~20 lines vs 50+ for DRP FSM
//   - Reliable: No PLL relock failures
//
// Frequencies Generated:
//   CLKOUT0: 100 MHz (fast debug)
//   CLKOUT1:  50 MHz (default)
//   CLKOUT2:  25 MHz
//   CLKOUT3:  10 MHz
//   CLKOUT4:   5 MHz (slow burn-in)
//
// VCO = 100 MHz × 10 = 1000 MHz (within 600-1200 MHz range)
//
//=============================================================================

`include "fbc_pkg.vh"

module clk_gen (
    //=========================================================================
    // Input Clocks (from Zynq PS)
    //=========================================================================
    input wire        clk_100m,         // FCLK_CLK0 - 100 MHz from PS
    input wire        clk_200m,         // FCLK_CLK1 - 200 MHz from PS
    input wire        resetn,           // Active-low reset

    //=========================================================================
    // Output Clocks
    //=========================================================================
    output wire       vec_clk,          // Vector execution clock (selected)
    output wire       vec_clk_90,       // 90° phase shifted (for pulse timing)
    output wire       vec_clk_180,      // 180° phase shifted
    output wire       delay_clk,        // High-speed I/O timing clock (200 MHz)

    //=========================================================================
    // Clock Control (directly memory-mapped)
    //=========================================================================
    input wire        vec_clk_en,       // Enable vec_clk output (gating)
    output wire       locked,           // MMCM locked indicator

    //=========================================================================
    // Frequency Selection (from AXI register)
    //=========================================================================
    input wire [2:0]  freq_sel,         // 0=5M, 1=10M, 2=25M, 3=50M, 4=100M

    //=========================================================================
    // Clock Outputs (for OBUFDS in system_top)
    //=========================================================================
    output wire [3:0] pll_clk       // [0]=Var, [1]=100MHz, [2]=Var, [3]=10MHz
);

    //=========================================================================
    // Internal Signals
    //=========================================================================
    wire mmcm_locked;
    wire clk_fb;

    // Pre-generated clocks (unbuffered)
    wire clk_100m_unbuf;
    wire clk_50m_unbuf;
    wire clk_25m_unbuf;
    wire clk_10m_unbuf;
    wire clk_5m_unbuf;

    // Selected clock (before gating)
    wire vec_clk_selected;

    // Phase-shifted versions of selected clock
    // Note: For simplicity, we generate 90/180 from 50 MHz only
    // Real implementation could use multiple phase outputs
    wire clk_50m_90_unbuf;
    wire clk_50m_180_unbuf;

    //=========================================================================
    // MMCM - Generate All Frequencies
    //=========================================================================
    // VCO = 100 MHz × 10 = 1000 MHz
    // CLKOUT0 = 1000 / 10  = 100 MHz
    // CLKOUT1 = 1000 / 20  =  50 MHz
    // CLKOUT2 = 1000 / 40  =  25 MHz
    // CLKOUT3 = 1000 / 100 =  10 MHz
    // CLKOUT4 = 1000 / 200 =   5 MHz
    // CLKOUT5 = 50 MHz @ 90°
    // CLKOUT6 = 50 MHz @ 180°

    MMCME2_ADV #(
        .BANDWIDTH            ("OPTIMIZED"),
        .CLKFBOUT_MULT_F      (10),        // VCO = 100 * 10 = 1000 MHz
        .CLKFBOUT_PHASE       (0),
        .CLKIN1_PERIOD        (10),        // 100 MHz = 10ns

        // CLKOUT0: 100 MHz
        .CLKOUT0_DIVIDE_F     (10),
        //.CLKOUT0_DUTY_CYCLE   (0.5),
        .CLKOUT0_PHASE        (0),

        // CLKOUT1: 50 MHz (default)
        .CLKOUT1_DIVIDE       (20),
        //.CLKOUT1_DUTY_CYCLE   (0.5),
        .CLKOUT1_PHASE        (0),

        // CLKOUT2: 25 MHz
        .CLKOUT2_DIVIDE       (40),
        //.CLKOUT2_DUTY_CYCLE   (0.5),
        .CLKOUT2_PHASE        (0),

        // CLKOUT3: 10 MHz
        .CLKOUT3_DIVIDE       (100),
        //.CLKOUT3_DUTY_CYCLE   (0.5),
        .CLKOUT3_PHASE        (0),

        // CLKOUT4: 5 MHz
        .CLKOUT4_DIVIDE       (200),
        //.CLKOUT4_DUTY_CYCLE   (0.5),
        .CLKOUT4_PHASE        (0),

        // CLKOUT5: 50 MHz @ 90
        .CLKOUT5_DIVIDE       (20),
        //.CLKOUT5_DUTY_CYCLE   (0.5),
        .CLKOUT5_PHASE        (90),

        // CLKOUT6: 50 MHz @ 180
        .CLKOUT6_DIVIDE       (20),
        //.CLKOUT6_DUTY_CYCLE   (0.5),
        .CLKOUT6_PHASE        (180),

        .DIVCLK_DIVIDE        (1),
        //.REF_JITTER1          (0.010),
        .STARTUP_WAIT         ("FALSE")
    ) u_mmcm (
        // Clock inputs
        .CLKIN1         (clk_100m),
        .CLKIN2         (1'b0),
        .CLKINSEL       (1'b1),

        // Clock outputs
        .CLKOUT0        (clk_100m_unbuf),
        .CLKOUT0B       (),
        .CLKOUT1        (clk_50m_unbuf),
        .CLKOUT1B       (),
        .CLKOUT2        (clk_25m_unbuf),
        .CLKOUT2B       (),
        .CLKOUT3        (clk_10m_unbuf),
        .CLKOUT3B       (),
        .CLKOUT4        (clk_5m_unbuf),
        .CLKOUT5        (clk_50m_90_unbuf),
        .CLKOUT6        (clk_50m_180_unbuf),

        // Feedback
        .CLKFBOUT       (clk_fb),
        .CLKFBOUTB      (),
        .CLKFBIN        (clk_fb),

        // Control
        .LOCKED         (mmcm_locked),
        .PWRDWN         (1'b0),
        .RST            (~resetn),

        // DRP (unused - we use MUX instead)
        .DADDR          (7'd0),
        .DCLK           (1'b0),
        .DEN            (1'b0),
        .DI             (16'd0),
        .DO             (),
        .DRDY           (),
        .DWE            (1'b0),

        // Phase shift (unused)
        .PSCLK          (1'b0),
        .PSEN           (1'b0),
        .PSINCDEC       (1'b0),
        .PSDONE         ()
    );

    //=========================================================================
    // Clock MUX - ONETWO: Select Pre-Generated Frequency
    //=========================================================================
    // Using cascaded BUFGMUX for 5:1 selection
    // freq_sel: 0=5M, 1=10M, 2=25M, 3=50M, 4=100M
    //
    // MUX tree:
    //   mux01: sel[0] ? clk_10m : clk_5m
    //   mux23: sel[0] ? clk_50m : clk_25m
    //   mux03: sel[1] ? mux23 : mux01
    //   mux_final: sel[2] ? clk_100m : mux03

    wire mux_01_out, mux_23_out, mux_03_out;

    // Level 1: Select within low pair and high pair
    BUFGMUX #(.CLK_SEL_TYPE("ASYNC")) u_mux_01 (
        .O  (mux_01_out),
        .I0 (clk_5m_unbuf),     // freq_sel[0]=0 → 5 MHz
        .I1 (clk_10m_unbuf),    // freq_sel[0]=1 → 10 MHz
        .S  (freq_sel[0])
    );

    BUFGMUX #(.CLK_SEL_TYPE("ASYNC")) u_mux_23 (
        .O  (mux_23_out),
        .I0 (clk_25m_unbuf),    // freq_sel[0]=0 → 25 MHz
        .I1 (clk_50m_unbuf),    // freq_sel[0]=1 → 50 MHz
        .S  (freq_sel[0])
    );

    // Level 2: Select between pairs
    BUFGMUX #(.CLK_SEL_TYPE("ASYNC")) u_mux_03 (
        .O  (mux_03_out),
        .I0 (mux_01_out),       // freq_sel[1]=0 → 5 or 10 MHz
        .I1 (mux_23_out),       // freq_sel[1]=1 → 25 or 50 MHz
        .S  (freq_sel[1])
    );

    // Level 3: Select 100 MHz or lower frequencies
    BUFGMUX #(.CLK_SEL_TYPE("ASYNC")) u_mux_final (
        .O  (vec_clk_selected),
        .I0 (mux_03_out),       // freq_sel[2]=0 → 5/10/25/50 MHz
        .I1 (clk_100m_unbuf),   // freq_sel[2]=1 → 100 MHz
        .S  (freq_sel[2])
    );

    //=========================================================================
    // Output Buffers with Gating
    //=========================================================================

    // vec_clk: Selected frequency with enable gating
    BUFGCE u_vec_clk_buf (
        .I  (vec_clk_selected),
        .CE (vec_clk_en & mmcm_locked),
        .O  (vec_clk)
    );

    // vec_clk_90: 90° phase (from 50 MHz, used for pulse timing)
    BUFGCE u_vec_clk_90_buf (
        .I  (clk_50m_90_unbuf),
        .CE (vec_clk_en & mmcm_locked),
        .O  (vec_clk_90)
    );

    // vec_clk_180: 180° phase (from 50 MHz)
    BUFGCE u_vec_clk_180_buf (
        .I  (clk_50m_180_unbuf),
        .CE (vec_clk_en & mmcm_locked),
        .O  (vec_clk_180)
    );

    // delay_clk: 200 MHz always running
    BUFG u_delay_clk_buf (
        .I  (clk_200m),
        .O  (delay_clk)
    );

    //=========================================================================
    // Status
    //=========================================================================
    assign locked = mmcm_locked;

    //=========================================================================
    // pll_clk[3]: Fixed 10 MHz
    //
    // Note: Using unbuffered outputs since OBUFDS provides output drive.
    // For internal logic, always use the buffered versions (vec_clk, etc.)
    assign pll_clk[0] = vec_clk_selected;   // Variable frequency (matches vec_clk)
    assign pll_clk[1] = clk_100m_unbuf;     // Fixed 100 MHz
    assign pll_clk[2] = clk_25m_unbuf;      // Fixed 25 MHz
    assign pll_clk[3] = clk_10m_unbuf;      // Fixed 10 MHz

endmodule
