// Simple testbench to test the parser
module simple_tb;

    reg clk;
    reg [3:0] count;

    // Clock generation
    initial clk = 0;
    always #5 clk = ~clk;

    // Counter
    initial begin
        count = 0;
        #100;
        $display("Count = %d", count);
        $finish;
    end

    always @(posedge clk) begin
        count <= count + 1;
    end

endmodule
