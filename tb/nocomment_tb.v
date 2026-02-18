module nocomment_tb;

    reg clk;
    reg [3:0] count;

    initial clk = 0;
    always #5 clk = ~clk;

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
