`timescale 1ns / 1ps
//=============================================================================
// Clock Generator Testbench
//=============================================================================

`include "fbc_pkg.vh"

module clk_gen_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_100M_PERIOD = 10;   // 100 MHz
    parameter CLK_200M_PERIOD = 5;    // 200 MHz

    //=========================================================================
    // Signals
    //=========================================================================
    reg        clk_100m;
    reg        clk_200m;
    reg        resetn;
    reg        vec_clk_en;
    reg [7:0]  clk_div;
    reg        reconfig_en;

    wire       vec_clk;
    wire       vec_clk_90;
    wire       vec_clk_180;
    wire       delay_clk;
    wire       locked;

    //=========================================================================
    // DUT
    //=========================================================================
    clk_gen u_clk_gen (
        .clk_100m       (clk_100m),
        .clk_200m       (clk_200m),
        .resetn         (resetn),
        .vec_clk        (vec_clk),
        .vec_clk_90     (vec_clk_90),
        .vec_clk_180    (vec_clk_180),
        .delay_clk      (delay_clk),
        .vec_clk_en     (vec_clk_en),
        .locked         (locked),
        .clk_div        (clk_div),
        .reconfig_en    (reconfig_en)
    );

    //=========================================================================
    // Clock Generation
    //=========================================================================
    initial clk_100m = 0;
    always #(CLK_100M_PERIOD/2) clk_100m = ~clk_100m;

    initial clk_200m = 0;
    always #(CLK_200M_PERIOD/2) clk_200m = ~clk_200m;

    //=========================================================================
    // Test Counters
    //=========================================================================
    integer tests_passed = 0;
    integer tests_failed = 0;

    // Clock edge counters
    integer vec_clk_edges = 0;
    integer delay_clk_edges = 0;

    always @(posedge vec_clk) vec_clk_edges = vec_clk_edges + 1;
    always @(posedge delay_clk) delay_clk_edges = delay_clk_edges + 1;

    //=========================================================================
    // Frequency Measurement
    //=========================================================================
    time vec_clk_rise_time;
    real vec_clk_period;
    real vec_clk_freq;

    always @(posedge vec_clk) begin
        if (vec_clk_rise_time != 0) begin
            vec_clk_period = $time - vec_clk_rise_time;
            vec_clk_freq = 1000.0 / vec_clk_period;  // GHz -> MHz
        end
        vec_clk_rise_time = $time;
    end

    //=========================================================================
    // Test Sequence
    //=========================================================================
    initial begin
        $display("========================================");
        $display("Clock Generator Testbench");
        $display("========================================");

        // Initialize
        resetn = 0;
        vec_clk_en = 0;
        clk_div = 8'd16;
        reconfig_en = 0;

        // Reset
        #100;
        resetn = 1;

        //=====================================================================
        // Test 1: Wait for MMCM Lock
        //=====================================================================
        $display("\n--- Test 1: MMCM Lock ---");

        // Wait for lock with timeout
        fork
            begin
                wait(locked);
                $display("PASS: MMCM locked after %0t", $time);
                tests_passed = tests_passed + 1;
            end
            begin
                #10000;
                if (!locked) begin
                    $display("FAIL: MMCM did not lock within timeout");
                    tests_failed = tests_failed + 1;
                end
            end
        join_any
        disable fork;

        //=====================================================================
        // Test 2: Clock Gating
        //=====================================================================
        $display("\n--- Test 2: Clock Gating ---");

        // Clocks should be stopped when vec_clk_en = 0
        vec_clk_edges = 0;
        #500;

        if (vec_clk_edges == 0) begin
            $display("PASS: vec_clk gated when disabled");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: vec_clk running when disabled (%0d edges)", vec_clk_edges);
            tests_failed = tests_failed + 1;
        end

        // Enable clocks
        vec_clk_en = 1;
        vec_clk_edges = 0;
        #500;

        if (vec_clk_edges > 0) begin
            $display("PASS: vec_clk running when enabled (%0d edges)", vec_clk_edges);
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: vec_clk not running when enabled");
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 3: delay_clk Always Running
        //=====================================================================
        $display("\n--- Test 3: delay_clk Always Running ---");

        delay_clk_edges = 0;
        vec_clk_en = 0;  // Disable vec_clk
        #500;

        if (delay_clk_edges > 0) begin
            $display("PASS: delay_clk running regardless of enable (%0d edges)", delay_clk_edges);
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: delay_clk not running");
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 4: Frequency Check
        //=====================================================================
        $display("\n--- Test 4: Frequency Check ---");

        vec_clk_en = 1;
        #2000;  // Let frequency measurement stabilize

        // Expected: 800 MHz VCO / 16 = 50 MHz vec_clk
        // Period = 20ns
        $display("  Measured vec_clk period: %0.1f ns", vec_clk_period);
        $display("  Measured vec_clk frequency: %0.1f MHz", vec_clk_freq);

        // Allow 10% tolerance for simulation timing
        if (vec_clk_period > 18 && vec_clk_period < 22) begin
            $display("PASS: vec_clk frequency within expected range");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: vec_clk frequency out of range (expected ~20ns period)");
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 5: Phase Relationships (qualitative)
        //=====================================================================
        $display("\n--- Test 5: Phase Relationships ---");

        // Sample vec_clk, vec_clk_90, vec_clk_180 at the same time
        // vec_clk_180 should be inverted from vec_clk

        @(posedge vec_clk);
        #1;  // Small delay to let signals settle

        if (vec_clk_180 === ~vec_clk) begin
            $display("PASS: vec_clk_180 is inverted from vec_clk");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: vec_clk_180 phase incorrect");
            tests_failed = tests_failed + 1;
        end

        // For 90 degree phase, we'd need more sophisticated measurement
        // Just verify it toggles
        @(posedge vec_clk_90);
        #1;
        @(negedge vec_clk_90);
        $display("PASS: vec_clk_90 is toggling");
        tests_passed = tests_passed + 1;

        //=====================================================================
        // Test 6: Reset Behavior
        //=====================================================================
        $display("\n--- Test 6: Reset Behavior ---");

        // Assert reset
        resetn = 0;
        #200;

        // Check that locked goes low
        if (!locked) begin
            $display("PASS: MMCM unlocked during reset");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: MMCM still locked during reset");
            tests_failed = tests_failed + 1;
        end

        // Release reset
        resetn = 1;

        // Wait for re-lock
        #500;
        if (locked) begin
            $display("PASS: MMCM re-locked after reset release");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: MMCM did not re-lock after reset");
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Results
        //=====================================================================
        $display("\n========================================");
        $display("Test Results: %0d passed, %0d failed", tests_passed, tests_failed);
        $display("========================================");

        if (tests_failed == 0)
            $display("ALL TESTS PASSED!");
        else
            $display("SOME TESTS FAILED!");

        $finish;
    end

    //=========================================================================
    // Timeout
    //=========================================================================
    initial begin
        #100000;
        $display("ERROR: Testbench timeout!");
        $finish;
    end

    //=========================================================================
    // Waveform Dump
    //=========================================================================
    initial begin
        $dumpfile("clk_gen_tb.vcd");
        $dumpvars(0, clk_gen_tb);
    end

endmodule
