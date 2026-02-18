`timescale 1ns / 1ps
// Debug test for parametric widths

module param_debug;
    parameter WIDTH = 128;
    parameter DOUBLE = WIDTH * 2;

    reg [WIDTH-1:0]         data128;  // Should be 128 bits
    reg [DOUBLE-1:0]        data256;  // Should be 256 bits

    initial begin
        $display("=== Parameter Width Debug ===");
        $display("WIDTH=%0d", WIDTH);
        $display("DOUBLE=%0d", DOUBLE);

        // Test 128-bit
        data128 = 128'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        $display("data128=%h", data128);

        // Test 256-bit
        data256 = 256'hFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;
        $display("data256=%h", data256);

        // Check high bits
        data128[127:120] = 8'hAB;
        $display("data128[127:120]=%h (expect ab)", data128[127:120]);

        data256[255:248] = 8'hCD;
        $display("data256[255:248]=%h (expect cd)", data256[255:248]);

        if (data128[127:120] == 8'hAB)
            $display("PASS: 128-bit works");
        else
            $display("FAIL: 128-bit broken");

        if (data256[255:248] == 8'hCD)
            $display("PASS: 256-bit works");
        else
            $display("FAIL: 256-bit broken");

        $finish;
    end
endmodule
