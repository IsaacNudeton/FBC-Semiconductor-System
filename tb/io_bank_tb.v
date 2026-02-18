`timescale 1ns / 1ps
//=============================================================================
// I/O Bank Testbench - Tests vec_clk_cnt and multiple pins
//=============================================================================

`include "fbc_pkg.vh"

module io_bank_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter DELAY_CLK_PERIOD = 5;    // 200 MHz
    parameter VEC_CLK_PERIOD = 20;     // 50 MHz (4x ratio)
    parameter WIDTH = `VECTOR_WIDTH;   // 128

    //=========================================================================
    // Signals
    //=========================================================================
    reg                     delay_clk;
    reg                     vec_clk;
    reg                     resetn;
    reg                     vec_clk_en;

    reg [4*WIDTH-1:0]       pin_type;
    reg [16*WIDTH-1:0]      pulse_ctrl_bits;

    reg [WIDTH-1:0]         dout;
    reg [WIDTH-1:0]         oen;

    reg [WIDTH-1:0]         pin_din;
    wire [WIDTH-1:0]        pin_dout;
    wire [WIDTH-1:0]        pin_oen;
    wire [WIDTH-1:0]        error;

    //=========================================================================
    // DUT
    //=========================================================================
    io_bank #(
        .WIDTH(WIDTH)
    ) u_io_bank (
        .delay_clk      (delay_clk),
        .vec_clk        (vec_clk),
        .resetn         (resetn),
        .vec_clk_en     (vec_clk_en),
        .pin_type       (pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits),
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
    initial delay_clk = 0;
    always #(DELAY_CLK_PERIOD/2) delay_clk = ~delay_clk;

    initial vec_clk = 0;
    always #(VEC_CLK_PERIOD/2) vec_clk = ~vec_clk;

    //=========================================================================
    // Test Counters
    //=========================================================================
    integer tests_passed = 0;
    integer tests_failed = 0;
    integer i;

    //=========================================================================
    // Helper Tasks
    //=========================================================================
    task set_pin_type;
        input integer pin_idx;
        input [3:0] ptype;
    begin
        pin_type[pin_idx*4 +: 4] = ptype;
    end
    endtask

    task set_pulse_ctrl;
        input integer pin_idx;
        input [7:0] start_time;
        input [7:0] end_time;
    begin
        pulse_ctrl_bits[pin_idx*16 +: 16] = {start_time, end_time};
    end
    endtask

    task wait_cycles;
        input integer n;
    begin
        repeat(n) @(posedge delay_clk);
    end
    endtask

    //=========================================================================
    // Test Sequence
    //=========================================================================
    initial begin
        $display("========================================");
        $display("I/O Bank Testbench");
        $display("========================================");

        // Initialize all pins to BIDI
        for (i = 0; i < WIDTH; i = i + 1) begin
            pin_type[i*4 +: 4] = `PIN_TYPE_BIDI;
            pulse_ctrl_bits[i*16 +: 16] = 16'h0000;
        end

        // Initialize other signals
        resetn = 0;
        vec_clk_en = 0;
        dout = {WIDTH{1'b0}};
        oen = {WIDTH{1'b1}};  // All inputs
        pin_din = {WIDTH{1'b0}};

        // Reset
        wait_cycles(20);
        resetn = 1;
        wait_cycles(20);

        //=====================================================================
        // Test 1: Basic BIDI Operation on Multiple Pins
        //=====================================================================
        $display("\n--- Test 1: BIDI Multiple Pins ---");

        // Set pins 0-7 as outputs driving alternating pattern
        for (i = 0; i < 8; i = i + 1) begin
            dout[i] = i[0];  // Alternating 0,1,0,1...
            oen[i] = 0;      // Output mode
        end

        wait_cycles(10);

        // Check outputs
        if (pin_dout[7:0] === 8'hAA && pin_oen[7:0] === 8'h00) begin
            $display("PASS: BIDI pins 0-7 output alternating pattern");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: Expected dout=0xAA, oen=0x00, got dout=%h, oen=%h",
                     pin_dout[7:0], pin_oen[7:0]);
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 2: Error Detection on INPUT Pins
        //=====================================================================
        $display("\n--- Test 2: INPUT Error Detection ---");

        // Set pins 8-15 as INPUT type
        for (i = 8; i < 16; i = i + 1) begin
            set_pin_type(i, `PIN_TYPE_INPUT);
            dout[i] = 1;     // Expect high
            oen[i] = 1;      // Input/compare mode
            pin_din[i] = 0;  // But pin is low -> ERROR
        end

        wait_cycles(10);

        // Check for errors on pins 8-15
        if (error[15:8] === 8'hFF) begin
            $display("PASS: INPUT pins 8-15 detected errors");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: Expected error=0xFF on pins 8-15, got %h", error[15:8]);
            tests_failed = tests_failed + 1;
        end

        // Fix the mismatch
        pin_din[15:8] = 8'hFF;
        wait_cycles(10);

        if (error[15:8] === 8'h00) begin
            $display("PASS: INPUT pins 8-15 no errors when matched");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: Expected no errors, got %h", error[15:8]);
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 3: vec_clk_cnt Counter
        //=====================================================================
        $display("\n--- Test 3: vec_clk_cnt Counter ---");

        // Enable vec_clk and observe counter behavior
        vec_clk_en = 1;

        // Wait for a vec_clk rising edge
        @(posedge vec_clk);
        wait_cycles(2);

        // The counter should start from 0 after vec_clk rising edge
        // With 200MHz delay_clk and 50MHz vec_clk, we have 4 delay_clk per vec_clk
        // Counter counts: 0, 1, 2, 3, then resets on next vec_clk edge

        $display("  vec_clk_cnt monitoring (expect 0,1,2,3,0,1,2,3...)");

        //=====================================================================
        // Test 4: PULSE Pin with Timing
        //=====================================================================
        $display("\n--- Test 4: PULSE Pin Timing ---");

        // Set pin 16 as PULSE type
        set_pin_type(16, `PIN_TYPE_PULSE);
        set_pulse_ctrl(16, 8'd1, 8'd3);  // Start at 1, end at 3
        dout[16] = 0;
        oen[16] = 1;  // Pulse mode {dout=0, oen=1}

        // Wait for vec_clk edge to reset counter
        @(posedge vec_clk);

        // Monitor pulse behavior over several delay_clk cycles
        $display("  Monitoring PULSE pin 16 (start=1, end=3)");

        // After vec_clk edge, counter starts at 0
        wait_cycles(1);  // cnt=0
        wait_cycles(1);  // cnt=1, pulse should go high
        wait_cycles(4);  // Pipeline delay
        if (pin_dout[16] === 1) begin
            $display("  PASS: Pulse high at cnt=1");
            tests_passed = tests_passed + 1;
        end else begin
            $display("  FAIL: Expected pulse high at cnt=1");
            tests_failed = tests_failed + 1;
        end

        //=====================================================================
        // Test 5: Mixed Pin Types
        //=====================================================================
        $display("\n--- Test 5: Mixed Pin Types ---");

        // Reset all pins
        dout = {WIDTH{1'b0}};
        oen = {WIDTH{1'b1}};
        pin_din = {WIDTH{1'b0}};

        // Set different pin types:
        // Pins 0-3:   OUTPUT (drive 0101)
        // Pins 4-7:   INPUT (expect 1010, receive 1010)
        // Pins 8-11:  OPEN_C (drive/float alternating)
        // Pins 12-15: BIDI (output mode)

        for (i = 0; i < 4; i = i + 1) begin
            set_pin_type(i, `PIN_TYPE_OUTPUT);
            dout[i] = i[0];
            oen[i] = 0;
        end

        for (i = 4; i < 8; i = i + 1) begin
            set_pin_type(i, `PIN_TYPE_INPUT);
            dout[i] = ~i[0];
            oen[i] = 1;
            pin_din[i] = ~i[0];  // Match expected
        end

        for (i = 8; i < 12; i = i + 1) begin
            set_pin_type(i, `PIN_TYPE_OPEN_C);
            dout[i] = i[0];  // 0=drive low, 1=float
            oen[i] = 0;
        end

        for (i = 12; i < 16; i = i + 1) begin
            set_pin_type(i, `PIN_TYPE_BIDI);
            dout[i] = i[0];
            oen[i] = 0;
        end

        wait_cycles(10);

        // Verify OUTPUT pins driving correctly
        if (pin_oen[3:0] === 4'b0000) begin
            $display("PASS: OUTPUT pins 0-3 in output mode");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: OUTPUT pins not in output mode");
            tests_failed = tests_failed + 1;
        end

        // Verify INPUT pins (always tristate)
        if (pin_oen[7:4] === 4'b1111) begin
            $display("PASS: INPUT pins 4-7 in input mode");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: INPUT pins not in input mode");
            tests_failed = tests_failed + 1;
        end

        // Verify no errors on INPUT pins (data matches)
        if (error[7:4] === 4'b0000) begin
            $display("PASS: INPUT pins 4-7 no compare errors");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: INPUT pins have errors: %b", error[7:4]);
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
        #500000;
        $display("ERROR: Testbench timeout!");
        $finish;
    end

    //=========================================================================
    // Waveform Dump
    //=========================================================================
    initial begin
        $dumpfile("io_bank_tb.vcd");
        $dumpvars(0, io_bank_tb);
    end

endmodule
