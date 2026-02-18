`timescale 1ns / 1ps
//=============================================================================
// I/O Bank - 160 Pin Controller (128 BIM + 32 Fast)
//=============================================================================
//
// Based on 2016 reference: reference/kzhang_v2_2016/io_table.v
//
// Architecture:
//   gpio[0:127]   - BIM-compatible pins (through Quad Board to DUT)
//   gpio[128:159] - Fast vector pins (direct FPGA, no BIM latency)
//
// Features:
// - All 160 pins individually configurable (type, timing)
// - vec_clk_cnt counter for precise pulse timing
// - Fast pins can run at full delay_clk speed
// - Pipelined for timing closure at 200MHz+
//
//=============================================================================

`include "fbc_pkg.vh"

module io_bank #(
    parameter BIM_WIDTH  = `VECTOR_WIDTH,  // 128 BIM pins
    parameter FAST_WIDTH = `FAST_WIDTH,    // 32 fast pins
    parameter WIDTH      = `PIN_COUNT      // 160 total
)(
    //=========================================================================
    // Clocks and Reset
    //=========================================================================
    input wire        delay_clk,        // Fast clock for timing (e.g., 200 MHz)
    input wire        vec_clk,          // Vector execution clock
    input wire        resetn,

    //=========================================================================
    // Timing Control
    //=========================================================================
    input wire        vec_clk_en,       // Vector clock enable (from engine)

    //=========================================================================
    // Pin Type Configuration (from AXI registers)
    //=========================================================================
    input wire [4*WIDTH-1:0]     pin_type,        // 4 bits per pin
    input wire [16*WIDTH-1:0]    pulse_ctrl_bits, // 16 bits per pin (start/end)

    //=========================================================================
    // Vector Data - BIM Pins (from FBC decoder/vector engine)
    //=========================================================================
    input wire [BIM_WIDTH-1:0]   dout,            // Drive values (128)
    input wire [BIM_WIDTH-1:0]   oen,             // Output enables (128)

    //=========================================================================
    // Vector Data - Fast Pins (direct control, no BIM)
    //=========================================================================
    input wire [FAST_WIDTH-1:0]  fast_dout,       // Fast pin drive (32)
    input wire [FAST_WIDTH-1:0]  fast_oen,        // Fast pin OE (32)
    input wire                   fast_clk_en,     // Fast pins clock enable

    //=========================================================================
    // Physical Pin Interface - All 160 Pins
    //=========================================================================
    input wire [WIDTH-1:0]       pin_din,         // Input from pins
    output wire [WIDTH-1:0]      pin_dout,        // Output to pins
    output wire [WIDTH-1:0]      pin_oen,         // Tristate control

    //=========================================================================
    // Error Output
    //=========================================================================
    output wire [BIM_WIDTH-1:0]  error,           // BIM pin error flags
    output wire [FAST_WIDTH-1:0] fast_error       // Fast pin error flags
);

    //=========================================================================
    // vec_clk_cnt - Counts cycles within each vector clock period
    //=========================================================================
    // This counter runs on delay_clk and resets on each rising edge of vec_clk.
    // Used by PULSE/NPULSE/VEC_CLK pin types for precise edge timing.
    //
    // Example: if delay_clk = 200MHz and vec_clk = 50MHz (4x ratio)
    //   vec_clk_cnt cycles: 0, 1, 2, 3, 0, 1, 2, 3, ...
    //   pulse_ctrl_bits[15:8] = 1 means pulse starts at count 1
    //   pulse_ctrl_bits[7:0] = 3 means pulse ends at count 3
    //=========================================================================

    reg [7:0] vec_clk_cnt;
    reg       vec_clk_d1;

    always @(posedge delay_clk or negedge resetn) begin
        if (!resetn) begin
            vec_clk_cnt <= 8'd0;
            vec_clk_d1  <= 1'b0;
        end else begin
            vec_clk_d1 <= vec_clk;

            // Reset counter on rising edge of vec_clk
            if (!vec_clk_d1 && vec_clk) begin
                vec_clk_cnt <= 8'd0;
            end else begin
                vec_clk_cnt <= vec_clk_cnt + 1'b1;
            end
        end
    end

    //=========================================================================
    // Registered vec_clk_en (for synchronization)
    //=========================================================================
    // synthesis attribute ASYNC_REG of vec_clk_en_sync is "TRUE"
    reg vec_clk_en_sync;

    always @(posedge delay_clk) begin
        vec_clk_en_sync <= vec_clk_en;
    end

    //=========================================================================
    // I/O Cell Instantiation - BIM Pins (0-127)
    //=========================================================================
    wire [BIM_WIDTH-1:0] bim_pin_dout;
    wire [BIM_WIDTH-1:0] bim_pin_oen;
    wire [BIM_WIDTH-1:0] bim_error;

    genvar i;
    generate
        for (i = 0; i < BIM_WIDTH; i = i + 1) begin : bim_cells

            io_cell u_io_cell (
                .clk            (delay_clk),
                .resetn         (resetn),

                // Configuration
                .pin_type       (pin_type[i*4 +: 4]),
                .pulse_ctrl_bits(pulse_ctrl_bits[i*16 +: 16]),

                // Timing
                .vec_clk_cnt    (vec_clk_cnt),

                // Vector data
                .dout           (dout[i]),
                .oen            (oen[i]),

                // Physical pin
                .pin_din        (pin_din[i]),
                .pin_dout       (bim_pin_dout[i]),
                .pin_oen        (bim_pin_oen[i]),

                // Error
                .error          (bim_error[i])
            );

        end
    endgenerate

    assign pin_dout[BIM_WIDTH-1:0] = bim_pin_dout;
    assign pin_oen[BIM_WIDTH-1:0]  = bim_pin_oen;
    assign error = bim_error;

    //=========================================================================
    // I/O Cell Instantiation - Fast Pins (128-159)
    //=========================================================================
    // Fast pins run with minimal latency - single cycle response
    // Can be used for: clocks, triggers, high-speed handshake, scope triggers
    //=========================================================================
    wire [FAST_WIDTH-1:0] fast_pin_dout;
    wire [FAST_WIDTH-1:0] fast_pin_oen;
    wire [FAST_WIDTH-1:0] fast_err;

    generate
        for (i = 0; i < FAST_WIDTH; i = i + 1) begin : fast_cells

            io_cell #(
                .FAST_MODE(1)  // Enable fast mode - reduced pipeline
            ) u_fast_cell (
                .clk            (delay_clk),
                .resetn         (resetn),

                // Configuration
                .pin_type       (pin_type[(BIM_WIDTH + i)*4 +: 4]),
                .pulse_ctrl_bits(pulse_ctrl_bits[(BIM_WIDTH + i)*16 +: 16]),

                // Timing
                .vec_clk_cnt    (vec_clk_cnt),

                // Vector data
                .dout           (fast_dout[i]),
                .oen            (fast_oen[i]),

                // Physical pin
                .pin_din        (pin_din[BIM_WIDTH + i]),
                .pin_dout       (fast_pin_dout[i]),
                .pin_oen        (fast_pin_oen[i]),

                // Error
                .error          (fast_err[i])
            );

        end
    endgenerate

    assign pin_dout[WIDTH-1:BIM_WIDTH] = fast_pin_dout;
    assign pin_oen[WIDTH-1:BIM_WIDTH]  = fast_pin_oen;
    assign fast_error = fast_err;

endmodule
