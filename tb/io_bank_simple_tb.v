`timescale 1ns / 1ps
//=============================================================================
// Simplified I/O Bank Testbench - Tests single pin propagation
//=============================================================================

module io_bank_simple_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter DELAY_CLK_PERIOD = 5;    // 200 MHz
    parameter WIDTH = 8;               // Reduced width for testing

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
    always #20 vec_clk = ~vec_clk;

    //=========================================================================
    // Test Counters
    //=========================================================================
    integer tests_passed = 0;
    integer tests_failed = 0;

    //=========================================================================
    // Test Sequence
    //=========================================================================
    initial begin
        $display("========================================");
        $display("Simplified I/O Bank Testbench");
        $display("========================================");

        // Initialize
        resetn = 0;
        vec_clk_en = 0;
        pin_type = 32'h00000000;     // All BIDI
        pulse_ctrl_bits = 128'd0;
        dout = 8'h00;
        oen = 8'hFF;  // All inputs
        pin_din = 8'h00;

        // Reset
        repeat(20) @(posedge delay_clk);
        resetn = 1;
        repeat(20) @(posedge delay_clk);

        //=====================================================================
        // Test 1: Single pin output
        //=====================================================================
        $display("\n--- Test 1: Single pin output ---");

        // Set pin 0 as output, drive high
        dout[0] = 1;
        oen[0] = 0;

        // Wait for pipeline
        repeat(10) @(posedge delay_clk);

        $display("dout=%b, oen=%b", dout, oen);
        $display("pin_dout=%b, pin_oen=%b", pin_dout, pin_oen);

        if (pin_dout[0] === 1 && pin_oen[0] === 0) begin
            $display("PASS: Pin 0 outputs high");
            tests_passed = tests_passed + 1;
        end else begin
            $display("FAIL: Expected pin_dout[0]=1, pin_oen[0]=0, got %b, %b",
                     pin_dout[0], pin_oen[0]);
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
        #10000;
        $display("ERROR: Testbench timeout!");
        $finish;
    end

endmodule
