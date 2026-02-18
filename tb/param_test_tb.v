`timescale 1ns / 1ps
`include "fbc_pkg.vh"

module param_test_tb;
    parameter CLK_PERIOD = 5;  // 200 MHz delay_clk
    reg        clk;
    reg        resetn;
    reg [3:0]  pin_type;
    reg [15:0] pulse_ctrl_bits;
    reg [7:0]  vec_clk_cnt;
    reg        dout;
    reg        oen;
    reg        pin_din;
    wire       pin_dout;
    wire       pin_oen;
    wire       error;
    integer tests_passed = 0;
    integer tests_failed = 0;

    // Module instantiation - exact format from io_cell_tb
    io_cell u_io_cell (
        .clk            (clk),
        .resetn         (resetn),
        .pin_type       (pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits),
        .vec_clk_cnt    (vec_clk_cnt),
        .dout           (dout),
        .oen            (oen),
        .pin_din        (pin_din),
        .pin_dout       (pin_dout),
        .pin_oen        (pin_oen),
        .error          (error)
    );

    // Task definition - same as io_cell_tb
    task check_output;
        input expected_dout;
        input expected_oen;
        input expected_error;
        input [127:0] test_name;
    begin
        repeat(4) @(posedge clk);
        if (pin_dout !== expected_dout || pin_oen !== expected_oen || error !== expected_error) begin
            $display("FAIL: %s", test_name);
            tests_failed = tests_failed + 1;
        end else begin
            $display("PASS: %s", test_name);
            tests_passed = tests_passed + 1;
        end
    end
    endtask

    initial clk = 0;
    always #5 clk = ~clk;

    initial begin
        resetn = 0;
        #100;
        resetn = 1;
        #100;
        $finish;
    end
endmodule
