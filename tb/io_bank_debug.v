`timescale 1ns / 1ps
// Debug io_bank signal propagation

module io_bank_debug;
    parameter WIDTH = 8;

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

    // Clock generation
    initial delay_clk = 0;
    always #2 delay_clk = ~delay_clk;

    initial vec_clk = 0;
    always #10 vec_clk = ~vec_clk;

    initial begin
        $display("=== IO Bank Debug Test ===");

        // Initialize
        resetn = 0;
        vec_clk_en = 0;
        pin_type = 32'h00000000;
        pulse_ctrl_bits = 128'd0;
        dout = 8'h00;
        oen = 8'hFF;
        pin_din = 8'h00;

        // Reset
        repeat(20) @(posedge delay_clk);
        resetn = 1;
        repeat(10) @(posedge delay_clk);

        $display("After reset:");
        $display("  TB dout=%h, oen=%h", dout, oen);
        $display("  pin_dout=%h, pin_oen=%h", pin_dout, pin_oen);

        // Set pin 0 as output driving high
        dout[0] = 1;
        oen[0] = 0;

        $display("\nAfter setting dout[0]=1, oen[0]=0:");
        $display("  TB dout=%h, oen=%h", dout, oen);

        // Wait for propagation
        repeat(20) @(posedge delay_clk);

        $display("\nAfter 20 clocks:");
        $display("  TB dout=%h, oen=%h", dout, oen);
        $display("  pin_dout=%h, pin_oen=%h", pin_dout, pin_oen);

        if (pin_dout[0] == 1 && pin_oen[0] == 0)
            $display("PASS");
        else
            $display("FAIL: Expected pin_dout[0]=1, pin_oen[0]=0");

        $finish;
    end
endmodule
