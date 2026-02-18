`timescale 1ns / 1ps
// Debug test for parametric width signals

module width_debug;
    parameter WIDTH = 128;

    reg [4*WIDTH-1:0]       pin_type;      // Should be 512 bits
    reg [16*WIDTH-1:0]      pulse_ctrl_bits; // Should be 2048 bits
    reg [WIDTH-1:0]         dout;          // Should be 128 bits
    reg [WIDTH-1:0]         oen;           // Should be 128 bits

    integer i;

    initial begin
        $display("=== Parametric Width Debug ===");
        $display("WIDTH=%0d", WIDTH);

        // Initialize
        pin_type = 512'd0;
        pulse_ctrl_bits = 2048'd0;
        dout = 128'd0;
        oen = 128'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;

        $display("Initial dout=%h", dout);
        $display("Initial oen=%h", oen);

        // Test 1: Set individual bits in wide signal
        for (i = 0; i < 8; i = i + 1) begin
            dout[i] = i[0];  // 0,1,0,1,0,1,0,1
            oen[i] = 0;
        end

        $display("After loop dout[7:0]=%h (expect aa)", dout[7:0]);
        $display("After loop oen[7:0]=%h (expect 00)", oen[7:0]);

        // Test 2: Set bits in upper portion
        for (i = 120; i < 128; i = i + 1) begin
            dout[i] = i[0];
        end

        $display("dout[127:120]=%h (expect aa)", dout[127:120]);

        // Test 3: Indexed part-select
        pin_type[3:0] = 4'hA;
        pin_type[7:4] = 4'hB;
        pin_type[511:508] = 4'hC;

        $display("pin_type[7:0]=%h (expect BA)", pin_type[7:0]);
        $display("pin_type[511:508]=%h (expect C)", pin_type[511:508]);

        if (dout[7:0] == 8'hAA && oen[7:0] == 8'h00)
            $display("PASS: Basic test");
        else
            $display("FAIL: Basic test");

        $finish;
    end
endmodule
