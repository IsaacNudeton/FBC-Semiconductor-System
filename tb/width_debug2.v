`timescale 1ns / 1ps
// Debug test for parametric width signals - simpler version

module width_debug2;
    parameter WIDTH = 128;

    // Use explicit widths to verify
    reg [511:0]             pin_type;      // Explicit 512 bits
    reg [127:0]             oen;           // Explicit 128 bits

    initial begin
        $display("=== Width Debug 2 ===");

        // Initialize with all 1s
        pin_type = 512'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        oen = 128'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;

        $display("Initial pin_type[7:0]=%h (expect ff)", pin_type[7:0]);
        $display("Initial pin_type[511:504]=%h (expect ff)", pin_type[511:504]);
        $display("Initial oen[7:0]=%h (expect ff)", oen[7:0]);
        $display("Initial oen[127:120]=%h (expect ff)", oen[127:120]);

        // Clear specific ranges
        pin_type[7:0] = 8'hAB;
        pin_type[511:504] = 8'hCD;
        oen[7:0] = 8'h00;
        oen[127:120] = 8'h55;

        $display("After assignment:");
        $display("  pin_type[7:0]=%h (expect ab)", pin_type[7:0]);
        $display("  pin_type[511:504]=%h (expect cd)", pin_type[511:504]);
        $display("  oen[7:0]=%h (expect 00)", oen[7:0]);
        $display("  oen[127:120]=%h (expect 55)", oen[127:120]);

        if (pin_type[511:504] == 8'hCD)
            $display("PASS: High bit range works");
        else
            $display("FAIL: High bit range broken");

        $finish;
    end
endmodule
