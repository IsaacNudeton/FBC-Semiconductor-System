`timescale 1ns / 1ps
//=============================================================================
// FBC Decoder Testbench
//=============================================================================

`include "../rtl/fbc_pkg.vh"

module fbc_decoder_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_PERIOD = 10;  // 100 MHz

    //=========================================================================
    // Signals
    //=========================================================================
    reg clk;
    reg resetn;

    // FBC input
    reg [63:0]  fbc_instr;
    reg [127:0] fbc_payload;
    reg         fbc_valid;
    wire        fbc_ready;

    // Vector output
    wire [127:0] vec_dout;
    wire [127:0] vec_oen;
    wire [31:0]  vec_repeat;
    wire         vec_valid;
    reg          vec_ready;

    // Status
    wire        running;
    wire        done;
    wire        error;
    wire [31:0] instr_count;
    wire [63:0] cycle_count;

    //=========================================================================
    // DUT
    //=========================================================================
    fbc_decoder #(
        .VECTOR_WIDTH(128),
        .REPEAT_WIDTH(32)
    ) dut (
        .clk(clk),
        .resetn(resetn),
        .fbc_instr(fbc_instr),
        .fbc_payload(fbc_payload),
        .fbc_valid(fbc_valid),
        .fbc_ready(fbc_ready),
        .vec_dout(vec_dout),
        .vec_oen(vec_oen),
        .vec_repeat(vec_repeat),
        .vec_valid(vec_valid),
        .vec_ready(vec_ready),
        .running(running),
        .done(done),
        .error(error),
        .instr_count(instr_count),
        .cycle_count(cycle_count)
    );

    //=========================================================================
    // Clock generation
    //=========================================================================
    initial clk = 0;
    always #(CLK_PERIOD/2) clk = ~clk;

    //=========================================================================
    // Helper tasks
    //=========================================================================

    // Build instruction word
    function [63:0] make_instr;
        input [7:0] opcode;
        input [7:0] flags;
        input [47:0] operand;
        begin
            make_instr = {opcode, flags, operand};
        end
    endfunction

    // Send instruction
    task send_instr;
        input [7:0] opcode;
        input [7:0] flags;
        input [47:0] operand;
        input [127:0] payload;
        begin
            @(posedge clk);
            fbc_instr <= make_instr(opcode, flags, operand);
            fbc_payload <= payload;
            fbc_valid <= 1'b1;

            // Wait for ready
            while (!fbc_ready) @(posedge clk);
            @(posedge clk);
            fbc_valid <= 1'b0;
        end
    endtask

    // Wait for vector output
    task wait_vec;
        begin
            while (!vec_valid) @(posedge clk);
            $display("Vector: dout=%h, oen=%h, repeat=%d",
                     vec_dout, vec_oen, vec_repeat);
            @(posedge clk);
            vec_ready <= 1'b1;
            @(posedge clk);
            vec_ready <= 1'b0;
        end
    endtask

    //=========================================================================
    // Test stimulus
    //=========================================================================
    initial begin
        $display("=== FBC Decoder Testbench ===");

        // Initialize
        resetn = 0;
        fbc_instr = 0;
        fbc_payload = 0;
        fbc_valid = 0;
        vec_ready = 0;

        // Reset
        repeat(10) @(posedge clk);
        resetn = 1;
        repeat(5) @(posedge clk);

        //=====================================================================
        // Test 1: SET_PINS
        //=====================================================================
        $display("\n--- Test 1: SET_PINS ---");
        send_instr(`FBC_SET_PINS, 8'h00, 48'h0, 128'hDEADBEEF_CAFEBABE_12345678_AABBCCDD);
        wait_vec();
        assert(vec_dout == 128'hDEADBEEF_CAFEBABE_12345678_AABBCCDD)
            else $error("SET_PINS failed");

        //=====================================================================
        // Test 2: SET_OEN
        //=====================================================================
        $display("\n--- Test 2: SET_OEN ---");
        send_instr(`FBC_SET_OEN, 8'h00, 48'h0, 128'hFFFF0000_FFFF0000_FFFF0000_FFFF0000);
        wait_vec();
        assert(vec_oen == 128'hFFFF0000_FFFF0000_FFFF0000_FFFF0000)
            else $error("SET_OEN failed");

        //=====================================================================
        // Test 3: PATTERN_REP (the big compression win)
        //=====================================================================
        $display("\n--- Test 3: PATTERN_REP (60000 cycles) ---");
        send_instr(`FBC_PATTERN_REP, 8'h00, 48'd60000, 128'h0);
        wait_vec();
        assert(vec_repeat == 32'd60000)
            else $error("PATTERN_REP failed, got repeat=%d", vec_repeat);
        $display("Compression: 1 instruction -> %d cycles", vec_repeat);

        //=====================================================================
        // Test 4: HALT
        //=====================================================================
        $display("\n--- Test 4: HALT ---");
        send_instr(`FBC_HALT, 8'h00, 48'h0, 128'h0);
        repeat(10) @(posedge clk);
        assert(done == 1'b1) else $error("HALT did not set done");

        //=====================================================================
        // Summary
        //=====================================================================
        $display("\n=== Test Summary ===");
        $display("Instructions executed: %d", instr_count);
        $display("Cycles generated: %d", cycle_count);
        $display("Compression ratio: %d:1", cycle_count / instr_count);

        if (!error) begin
            $display("\n*** ALL TESTS PASSED ***");
        end else begin
            $display("\n*** TESTS FAILED ***");
        end

        $finish;
    end

    //=========================================================================
    // Timeout
    //=========================================================================
    initial begin
        #100000;
        $display("ERROR: Timeout!");
        $finish;
    end

endmodule
