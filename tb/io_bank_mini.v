`timescale 1ns / 1ps
// Minimal io_bank test - just 4 pins

`include "fbc_pkg.vh"

module io_bank_mini;

    parameter WIDTH = 4;

    reg clk;
    reg resetn;

    reg [4*WIDTH-1:0]    pin_type;
    reg [16*WIDTH-1:0]   pulse_ctrl_bits;
    reg [WIDTH-1:0]      dout;
    reg [WIDTH-1:0]      oen;
    reg [WIDTH-1:0]      pin_din;
    wire [WIDTH-1:0]     pin_dout;
    wire [WIDTH-1:0]     pin_oen;
    wire [WIDTH-1:0]     error;

    // DUT with width=4
    io_bank #(.WIDTH(WIDTH)) u_io_bank (
        .delay_clk(clk),
        .vec_clk(clk),
        .resetn(resetn),
        .vec_clk_en(1'b0),
        .pin_type(pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits),
        .dout(dout),
        .oen(oen),
        .pin_din(pin_din),
        .pin_dout(pin_dout),
        .pin_oen(pin_oen),
        .error(error)
    );

    initial clk = 0;
    always #5 clk = ~clk;

    initial begin
        $display("=== IO Bank Mini Test (4 pins) ===");

        // Initialize
        resetn = 0;
        pin_type = {4{`PIN_TYPE_BIDI}};
        pulse_ctrl_bits = 64'h0;
        dout = 4'b0;
        oen = 4'b1111;
        pin_din = 4'b0;

        // Reset
        #20;
        resetn = 1;
        #50;  // Wait for pipeline

        $display("After reset: pin_dout=%b, pin_oen=%b", pin_dout, pin_oen);

        // Test: Set pin 0 to output mode, drive high
        dout[0] = 1;
        oen[0] = 0;

        #50;  // Wait for pipeline (10 clock cycles)

        $display("After setting pin0 output: pin_dout=%b, pin_oen=%b", pin_dout, pin_oen);

        // Check intermediate signals
        $display("Checking intermediate values...");
        $display("  dout=%b oen=%b", dout, oen);
        $display("  pin_type=%h", pin_type);

        // Expected: pin_dout[0]=1, pin_oen[0]=0
        if (pin_dout[0] == 1 && pin_oen[0] == 0) begin
            $display("PASS: Pin 0 output works");
        end else begin
            $display("FAIL: Expected pin_dout[0]=1, pin_oen[0]=0, got pin_dout=%b, pin_oen=%b", pin_dout, pin_oen);
        end

        // Test: Set all pins to output alternating pattern
        dout = 4'b1010;
        oen = 4'b0000;

        #50;

        $display("After alternating pattern: pin_dout=%b (expect 1010), pin_oen=%b (expect 0000)", pin_dout, pin_oen);

        if (pin_dout == 4'b1010 && pin_oen == 4'b0000) begin
            $display("PASS: All pins output works");
        end else begin
            $display("FAIL: Pattern mismatch");
        end

        $finish;
    end
endmodule
