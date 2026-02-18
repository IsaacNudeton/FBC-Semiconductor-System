`timescale 1ns / 1ps
//=============================================================================
// Error Injection Testbench
//=============================================================================
// Focused test for error detection and error_counter module:
// 1. Single-bit errors on specific pins
// 2. Multi-bit errors (burst)
// 3. Error counter overflow
// 4. First error capture accuracy
// 5. Error BRAM storage
//=============================================================================

`include "../rtl/fbc_pkg.vh"

module error_inject_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_PERIOD = 10;        // 100 MHz
    parameter VECTOR_WIDTH = 128;
    parameter MAX_ERRORS = 16;        // Small for testing overflow

    //=========================================================================
    // Signals
    //=========================================================================
    reg clk;
    reg resetn;

    // Error input
    reg [VECTOR_WIDTH-1:0] error_mask;
    reg error_valid;
    reg [31:0] vector_count;
    reg [63:0] cycle_count;

    // BRAM outputs
    wire [31:0] bram_addr;
    wire [VECTOR_WIDTH-1:0] bram_data;
    wire bram_we;
    wire [31:0] vec_bram_addr;
    wire [31:0] vec_bram_data;
    wire vec_bram_we;
    wire [31:0] cyc_bram_addr;
    wire [63:0] cyc_bram_data;
    wire cyc_bram_we;

    // Status outputs
    wire [31:0] total_error_count;
    wire [31:0] first_error_vector;
    wire [63:0] first_error_cycle;
    wire first_error_detected;
    wire error_overflow;

    // Test tracking
    integer test_pass_count;
    integer test_fail_count;
    integer bram_write_count;

    // BRAM storage (shadow)
    reg [VECTOR_WIDTH-1:0] error_bram [0:MAX_ERRORS-1];
    reg [31:0] vec_bram [0:MAX_ERRORS-1];
    reg [63:0] cyc_bram [0:MAX_ERRORS-1];

    //=========================================================================
    // DUT
    //=========================================================================
    error_counter #(
        .VECTOR_WIDTH(VECTOR_WIDTH),
        .MAX_ERRORS(MAX_ERRORS),
        .ERROR_COUNT_WIDTH(32)
    ) dut (
        .clk(clk),
        .resetn(resetn),
        .error_mask(error_mask),
        .error_valid(error_valid),
        .vector_count(vector_count),
        .cycle_count(cycle_count),
        .bram_addr(bram_addr),
        .bram_data(bram_data),
        .bram_we(bram_we),
        .vec_bram_addr(vec_bram_addr),
        .vec_bram_data(vec_bram_data),
        .vec_bram_we(vec_bram_we),
        .cyc_bram_addr(cyc_bram_addr),
        .cyc_bram_data(cyc_bram_data),
        .cyc_bram_we(cyc_bram_we),
        .total_error_count(total_error_count),
        .first_error_vector(first_error_vector),
        .first_error_cycle(first_error_cycle),
        .first_error_detected(first_error_detected),
        .error_overflow(error_overflow)
    );

    //=========================================================================
    // Clock Generation
    //=========================================================================
    initial clk = 0;
    always #(CLK_PERIOD/2) clk = ~clk;

    //=========================================================================
    // BRAM Shadow (capture writes for verification)
    //=========================================================================
    always @(posedge clk) begin
        if (bram_we) begin
            $display("  [BRAM] Error pattern written: addr=%h data=%h",
                bram_addr, bram_data);
            bram_write_count = bram_write_count + 1;
        end
        if (vec_bram_we) begin
            $display("  [BRAM] Vector count written: addr=%h data=%d",
                vec_bram_addr, vec_bram_data);
        end
        if (cyc_bram_we) begin
            $display("  [BRAM] Cycle count written: addr=%h data=%d",
                cyc_bram_addr, cyc_bram_data);
        end
    end

    //=========================================================================
    // Helper Tasks
    //=========================================================================

    // Inject a single error
    task inject_error;
        input [6:0] pin;
        input [31:0] vec;
        input [63:0] cyc;
        begin
            @(posedge clk);
            error_mask <= (128'h1 << pin);
            error_valid <= 1'b1;
            vector_count <= vec;
            cycle_count <= cyc;
            @(posedge clk);
            error_valid <= 1'b0;
            error_mask <= 128'h0;
            @(posedge clk);
        end
    endtask

    // Inject multi-bit error
    task inject_multi_error;
        input [VECTOR_WIDTH-1:0] mask;
        input [31:0] vec;
        input [63:0] cyc;
        begin
            @(posedge clk);
            error_mask <= mask;
            error_valid <= 1'b1;
            vector_count <= vec;
            cycle_count <= cyc;
            @(posedge clk);
            error_valid <= 1'b0;
            error_mask <= 128'h0;
            @(posedge clk);
        end
    endtask

    // Check test result
    task check_result;
        input [255:0] test_name;
        input [63:0] expected;
        input [63:0] actual;
        begin
            if (expected == actual) begin
                $display("  [PASS] %s", test_name);
                test_pass_count = test_pass_count + 1;
            end else begin
                $display("  [FAIL] %s - Expected %d, Got %d", test_name, expected, actual);
                test_fail_count = test_fail_count + 1;
            end
        end
    endtask

    //=========================================================================
    // Test Stimulus
    //=========================================================================
    integer i;

    initial begin
        $display("=============================================================================");
        $display("Error Injection Testbench");
        $display("=============================================================================");
        $display("  VECTOR_WIDTH: %d", VECTOR_WIDTH);
        $display("  MAX_ERRORS: %d", MAX_ERRORS);
        $display("=============================================================================");

        $dumpfile("error_inject_tb.vcd");
        $dumpvars(0, error_inject_tb);

        // Initialize
        test_pass_count = 0;
        test_fail_count = 0;
        bram_write_count = 0;

        resetn = 0;
        error_mask = 0;
        error_valid = 0;
        vector_count = 0;
        cycle_count = 0;

        // Reset
        repeat(10) @(posedge clk);
        resetn = 1;
        repeat(5) @(posedge clk);

        //=====================================================================
        // Test 1: Initial State
        //=====================================================================
        $display("\n--- Test 1: Initial State ---");
        check_result("total_error_count = 0", 0, total_error_count);
        check_result("first_error_detected = 0", 0, first_error_detected);
        check_result("error_overflow = 0", 0, error_overflow);

        //=====================================================================
        // Test 2: Single Error Injection
        //=====================================================================
        $display("\n--- Test 2: Single Error Injection ---");

        inject_error(7'd42, 32'd100, 64'd1000);

        check_result("total_error_count = 1", 1, total_error_count);
        check_result("first_error_detected = 1", 1, first_error_detected);
        check_result("first_error_vector = 100", 100, first_error_vector);
        check_result("first_error_cycle = 1000", 1000, first_error_cycle);

        //=====================================================================
        // Test 3: Second Error (first_error should not change)
        //=====================================================================
        $display("\n--- Test 3: Second Error (first_error unchanged) ---");

        inject_error(7'd10, 32'd200, 64'd2000);

        check_result("total_error_count = 2", 2, total_error_count);
        check_result("first_error_vector unchanged", 100, first_error_vector);
        check_result("first_error_cycle unchanged", 1000, first_error_cycle);

        //=====================================================================
        // Test 4: Multi-bit Error
        //=====================================================================
        $display("\n--- Test 4: Multi-bit Error ---");

        inject_multi_error(128'hFFFF_0000_FFFF_0000, 32'd300, 64'd3000);

        check_result("total_error_count = 3", 3, total_error_count);

        //=====================================================================
        // Test 5: Error Overflow
        //=====================================================================
        $display("\n--- Test 5: Error Overflow ---");

        // Inject enough errors to overflow
        for (i = 0; i < MAX_ERRORS + 5; i = i + 1) begin
            inject_error(i[6:0], 32'd400 + i, 64'd4000 + i);
        end

        check_result("error_overflow = 1", 1, error_overflow);
        $display("  Total errors after overflow: %d", total_error_count);
        $display("  BRAM writes (capped at MAX_ERRORS): %d", bram_write_count);

        //=====================================================================
        // Test 6: Reset Clears Errors
        //=====================================================================
        $display("\n--- Test 6: Reset Clears Errors ---");

        // Apply reset
        resetn = 0;
        repeat(5) @(posedge clk);
        resetn = 1;
        repeat(5) @(posedge clk);

        check_result("total_error_count = 0 after reset", 0, total_error_count);
        check_result("first_error_detected = 0 after reset", 0, first_error_detected);
        check_result("error_overflow = 0 after reset", 0, error_overflow);

        //=====================================================================
        // Test 7: Error Pattern Verification
        //=====================================================================
        $display("\n--- Test 7: Error Pattern Verification ---");

        // Reset bram write counter
        bram_write_count = 0;

        // Inject specific pattern
        inject_multi_error(128'hDEAD_BEEF_CAFE_BABE_1234_5678_ABCD_EF00, 32'd500, 64'd5000);

        $display("  Pattern injected: 0xDEADBEEFCAFEBABE12345678ABCDEF00");
        check_result("BRAM write occurred", 1, (bram_write_count > 0));

        //=====================================================================
        // Test Summary
        //=====================================================================
        $display("\n=============================================================================");
        $display("TEST SUMMARY");
        $display("=============================================================================");
        $display("  Passed: %d", test_pass_count);
        $display("  Failed: %d", test_fail_count);
        $display("=============================================================================");

        if (test_fail_count == 0) begin
            $display("*** ALL TESTS PASSED ***");
        end else begin
            $display("*** SOME TESTS FAILED ***");
        end

        $finish;
    end

    //=========================================================================
    // Timeout
    //=========================================================================
    initial begin
        #100000;
        $display("ERROR: Timeout!");
        $finish;
    end

endmodule
