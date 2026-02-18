`timescale 1ns / 1ps
// Debug test for for loop indexed assignments

module forloop_test;
    reg [7:0] dout;
    reg [7:0] oen;
    integer i;

    initial begin
        $display("=== For Loop Index Test ===");

        // Initialize
        dout = 8'h00;
        oen = 8'hFF;

        $display("Before loop: dout=%h, oen=%h", dout, oen);

        // Test 1: Simple indexed assignment in for loop
        for (i = 0; i < 8; i = i + 1) begin
            $display("  i=%0d, i[0]=%0d", i, i[0]);
            dout[i] = i[0];  // Alternating 0,1,0,1...
            oen[i] = 0;      // Output mode
        end

        $display("After loop: dout=%h (expect aa), oen=%h (expect 00)", dout, oen);

        if (dout == 8'hAA && oen == 8'h00)
            $display("PASS");
        else
            $display("FAIL");

        $finish;
    end
endmodule
