`timescale 1ns / 1ps
// Debug test for range assignments

module range_debug;
    reg [15:0] data;

    initial begin
        $display("=== Range Assignment Debug ===");

        // Initialize
        data = 16'hFFFF;
        $display("Initial data=%h (expect ffff)", data);

        // Range assignment
        data[7:0] = 8'hAB;
        $display("After data[7:0]=AB: data=%h (expect ffab)", data);

        data[15:8] = 8'h12;
        $display("After data[15:8]=12: data=%h (expect 12ab)", data);

        // Single bit
        data[0] = 0;
        $display("After data[0]=0: data=%h (expect 12aa)", data);

        if (data == 16'h12AA)
            $display("PASS");
        else
            $display("FAIL");

        $finish;
    end
endmodule
