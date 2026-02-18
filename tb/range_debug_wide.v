`timescale 1ns / 1ps
// Debug test for wide range assignments

module range_debug_wide;
    reg [127:0] data;

    initial begin
        $display("=== Wide Range Assignment Debug ===");

        // Initialize to all 1s
        data = 128'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        $display("Initial data[7:0]=%h (expect ff)", data[7:0]);
        $display("Initial data[127:120]=%h (expect ff)", data[127:120]);

        // Range assignment - low bits
        data[7:0] = 8'hAB;
        $display("After data[7:0]=AB: data[7:0]=%h (expect ab)", data[7:0]);

        // Range assignment - high bits
        data[127:120] = 8'h12;
        $display("After data[127:120]=12: data[127:120]=%h (expect 12)", data[127:120]);

        // Check values
        if (data[7:0] == 8'hAB && data[127:120] == 8'h12)
            $display("PASS");
        else
            $display("FAIL: data[7:0]=%h, data[127:120]=%h", data[7:0], data[127:120]);

        $finish;
    end
endmodule
