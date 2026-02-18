`timescale 1ns / 1ps
//=============================================================================
// I/O Cell Testbench - Tests all pin types
//=============================================================================

`include "fbc_pkg.vh"

module io_cell_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_PERIOD = 5;  // 200 MHz delay_clk

    //=========================================================================
    // Signals
    //=========================================================================
    reg        clk;
    reg        resetn;
    reg [3:0]  pin_type;
    reg [15:0] pulse_ctrl_bits;
    reg [7:0]  vec_clk_cnt;
    reg        dout;
    reg        oen;
    reg        pin_din;
    wire       pin_dout;
    wire       pin_oen;
    wire       error;

    //=========================================================================
    // DUT
    //=========================================================================
    io_cell u_io_cell (
        .clk            (clk),
        .resetn         (resetn),
        .pin_type       (pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits),
        .vec_clk_cnt    (vec_clk_cnt),
        .dout           (dout),
        .oen            (oen),
        .pin_din        (pin_din),
        .pin_dout       (pin_dout),
        .pin_oen        (pin_oen),
        .error          (error)
    );

    //=========================================================================
    // Clock Generation
    //=========================================================================
    initial clk = 0;
    always #(CLK_PERIOD/2) clk = ~clk;

    //=========================================================================
    // Test Counters
    //=========================================================================
    integer tests_passed = 0;
    integer tests_failed = 0;

    //=========================================================================
    // Tasks
    //=========================================================================
    task check_output;
        input expected_dout;
        input expected_oen;
        input expected_error;
        input [127:0] test_name;
    begin
        // Wait for pipeline (2 stages)
        repeat(4) @(posedge clk);

        if (pin_dout !== expected_dout || pin_oen !== expected_oen || error !== expected_error) begin
            $display("FAIL: %s", test_name);
            $display("  Expected: dout=%b, oen=%b, error=%b", expected_dout, expected_oen, expected_error);
            $display("  Got:      dout=%b, oen=%b, error=%b", pin_dout, pin_oen, error);
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: %s", test_name);
            tests_passed = tests_passed + 1;
        end
    end
    endtask

    //=========================================================================
    // Test Sequence
    //=========================================================================
    initial begin
        $display("========================================");
        $display("I/O Cell Testbench");
        $display("========================================");

        // Initialize
        resetn = 0;
        pin_type = `PIN_TYPE_BIDI;
        pulse_ctrl_bits = 16'h0000;
        vec_clk_cnt = 8'd0;
        dout = 0;
        oen = 1;
        pin_din = 0;

        // Reset
        repeat(10) @(posedge clk);
        resetn = 1;
        repeat(10) @(posedge clk);

        //=====================================================================
        // Test 1: BIDI Pin Type
        //=====================================================================
        $display("\n--- Test BIDI Pin Type ---");
        pin_type = `PIN_TYPE_BIDI;

        // Output mode (oen=0), drive high
        dout = 1; oen = 0; pin_din = 1;
        check_output(1, 0, 0, "BIDI: Output high, no error");

        // Output mode, drive low
        dout = 0; oen = 0; pin_din = 0;
        check_output(0, 0, 0, "BIDI: Output low, no error");

        // Input mode (oen=1), expect high, got high
        dout = 1; oen = 1; pin_din = 1;
        check_output(1, 1, 0, "BIDI: Input expect H, got H");

        // Input mode, expect high, got low (ERROR)
        dout = 1; oen = 1; pin_din = 0;
        check_output(1, 1, 1, "BIDI: Input expect H, got L - ERROR");

        //=====================================================================
        // Test 2: INPUT Pin Type
        //=====================================================================
        $display("\n--- Test INPUT Pin Type ---");
        pin_type = `PIN_TYPE_INPUT;

        // Always input mode, compare match
        dout = 1; oen = 1; pin_din = 1;
        check_output(1, 1, 0, "INPUT: Compare H match");

        dout = 0; oen = 1; pin_din = 0;
        check_output(0, 1, 0, "INPUT: Compare L match");

        // Compare mismatch (ERROR)
        dout = 1; oen = 1; pin_din = 0;
        check_output(1, 1, 1, "INPUT: Compare mismatch - ERROR");

        //=====================================================================
        // Test 3: OUTPUT Pin Type
        //=====================================================================
        $display("\n--- Test OUTPUT Pin Type ---");
        pin_type = `PIN_TYPE_OUTPUT;

        // Output mode, never error
        dout = 1; oen = 0; pin_din = 0;  // pin_din doesn't matter
        check_output(1, 0, 0, "OUTPUT: Drive high");

        dout = 0; oen = 0; pin_din = 1;
        check_output(0, 0, 0, "OUTPUT: Drive low");

        // Tristate mode
        dout = 0; oen = 1; pin_din = 0;
        check_output(0, 1, 0, "OUTPUT: Tristate");

        //=====================================================================
        // Test 4: OPEN_COLLECTOR Pin Type
        //=====================================================================
        $display("\n--- Test OPEN_COLLECTOR Pin Type ---");
        pin_type = `PIN_TYPE_OPEN_C;

        // Drive low (oen=0, dout=0 -> pin_oen=0)
        dout = 0; oen = 0;
        check_output(0, 0, 0, "OPEN_C: Drive low");

        // Float (dout=1 -> pin_oen=1)
        dout = 1; oen = 0;
        check_output(0, 1, 0, "OPEN_C: Float high");

        // Compare mode with error
        dout = 1; oen = 1; pin_din = 0;
        check_output(0, 1, 1, "OPEN_C: Compare mismatch - ERROR");

        //=====================================================================
        // Test 5: PULSE Pin Type
        //=====================================================================
        $display("\n--- Test PULSE Pin Type ---");
        pin_type = `PIN_TYPE_PULSE;
        pulse_ctrl_bits = 16'h0208;  // Start at count 2, end at count 8

        // Static low
        dout = 0; oen = 0;
        check_output(0, 0, 0, "PULSE: Static low");

        // Static high
        dout = 1; oen = 0;
        check_output(1, 0, 0, "PULSE: Static high");

        // Pulse mode - before start
        dout = 0; oen = 1; vec_clk_cnt = 8'd1;
        @(posedge clk);

        // Pulse mode - at start (should go high)
        vec_clk_cnt = 8'd2;
        repeat(4) @(posedge clk);
        if (pin_dout !== 1) begin
            $display("FAIL: PULSE: Should be high at start");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: PULSE: High at start time");
            tests_passed = tests_passed + 1;
        end

        // Pulse mode - at end (should go low)
        vec_clk_cnt = 8'd8;
        repeat(4) @(posedge clk);
        if (pin_dout !== 0) begin
            $display("FAIL: PULSE: Should be low at end");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: PULSE: Low at end time");
            tests_passed = tests_passed + 1;
        end

        //=====================================================================
        // Test 6: NPULSE Pin Type (inverted)
        //=====================================================================
        $display("\n--- Test NPULSE Pin Type ---");
        pin_type = `PIN_TYPE_NPULSE;
        pulse_ctrl_bits = 16'h0408;  // Start at count 4, end at count 8

        // NPULSE mode - at start (should go LOW, opposite of PULSE)
        dout = 0; oen = 1; vec_clk_cnt = 8'd4;
        repeat(4) @(posedge clk);
        if (pin_dout !== 0) begin
            $display("FAIL: NPULSE: Should be low at start");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: NPULSE: Low at start time");
            tests_passed = tests_passed + 1;
        end

        // NPULSE mode - at end (should go HIGH)
        vec_clk_cnt = 8'd8;
        repeat(4) @(posedge clk);
        if (pin_dout !== 1) begin
            $display("FAIL: NPULSE: Should be high at end");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: NPULSE: High at end time");
            tests_passed = tests_passed + 1;
        end

        //=====================================================================
        // Test 7: VEC_CLK Pin Type
        //=====================================================================
        $display("\n--- Test VEC_CLK Pin Type ---");
        pin_type = `PIN_TYPE_VEC_CLK;
        pulse_ctrl_bits = 16'h0010;  // High at 0, low at 16

        // At start - should go high
        vec_clk_cnt = 8'd0;
        repeat(4) @(posedge clk);
        if (pin_dout !== 1) begin
            $display("FAIL: VEC_CLK: Should be high at 0");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: VEC_CLK: High at count 0");
            tests_passed = tests_passed + 1;
        end

        // At end - should go low
        vec_clk_cnt = 8'd16;
        repeat(4) @(posedge clk);
        if (pin_dout !== 0) begin
            $display("FAIL: VEC_CLK: Should be low at 16");
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: VEC_CLK: Low at count 16");
            tests_passed = tests_passed + 1;
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

endmodule
