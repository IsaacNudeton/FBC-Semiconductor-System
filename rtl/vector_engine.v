`timescale 1ns / 1ps
//=============================================================================
// Vector Engine - Repeat Counter and Vector Data Routing
//=============================================================================
//
// Handles:
// - Repeat counter (expands compressed vectors)
// - Raw vector data output to io_bank
// - Error valid flag generation
// - Vector/cycle counting
//
// Note: Pin type processing and error detection moved to io_bank/io_cell
// for proper pulse timing on delay_clk domain.
//
//=============================================================================

`include "fbc_pkg.vh"

module vector_engine #(
    parameter VECTOR_WIDTH = `VECTOR_WIDTH,
    parameter REPEAT_WIDTH = `REPEAT_WIDTH
)(
    input wire clk,
    input wire vec_clk,           // Vector execution clock
    input wire resetn,

    //=========================================================================
    // Vector Input (from FBC decoder)
    //=========================================================================
    input wire [VECTOR_WIDTH-1:0] in_dout,       // Pin values to drive
    input wire [VECTOR_WIDTH-1:0] in_oen,        // Output enables (0=output)
    input wire [REPEAT_WIDTH-1:0] in_repeat,     // Repeat count
    input wire                    in_valid,      // Input valid
    output wire                   in_ready,      // Ready for input

    //=========================================================================
    // Pin Interface (directly connect pins here in physical impl)
    //=========================================================================
    output wire [VECTOR_WIDTH-1:0] pin_dout,     // To pin output drivers
    output wire [VECTOR_WIDTH-1:0] pin_oen,      // To pin tri-state control
    input wire [VECTOR_WIDTH-1:0]  pin_din,      // From pin input buffers

    //=========================================================================
    // Pin Type Configuration (from AXI registers)
    //=========================================================================
    input wire [4*VECTOR_WIDTH-1:0] pin_type,    // 4 bits per pin

    //=========================================================================
    // Error Interface
    //=========================================================================
    input wire [VECTOR_WIDTH-1:0]  error_mask,   // From io_bank: which pins have errors
    output wire                    error_valid,  // Error detected this cycle
    output wire [31:0]             vector_count, // Current vector number
    output wire [63:0]             cycle_count,  // Current cycle number

    //=========================================================================
    // Status
    //=========================================================================
    output wire                    running,      // Engine is running
    output wire                    done,         // All vectors complete
    input wire                     enable        // Enable engine
);

    //=========================================================================
    // Repeat Counter
    //=========================================================================
    reg [REPEAT_WIDTH-1:0] repeat_cnt;
    reg [VECTOR_WIDTH-1:0] current_dout;
    reg [VECTOR_WIDTH-1:0] current_oen;
    reg                    vec_active;

    wire repeat_done = (repeat_cnt == 0);
    wire load_new = in_valid && in_ready;

    // Ready when repeat counter is done or idle
    assign in_ready = repeat_done && enable;

    always @(posedge vec_clk or negedge resetn) begin
        if (!resetn) begin
            repeat_cnt <= 0;
            current_dout <= {VECTOR_WIDTH{1'b0}};
            current_oen <= {VECTOR_WIDTH{1'b1}};  // All inputs
            vec_active <= 1'b0;
        end else if (enable) begin
            if (load_new) begin
                // Load new vector
                repeat_cnt <= in_repeat - 1;  // -1 because we output immediately
                current_dout <= in_dout;
                current_oen <= in_oen;
                vec_active <= 1'b1;
            end else if (!repeat_done) begin
                // Count down
                repeat_cnt <= repeat_cnt - 1;
            end else begin
                vec_active <= 1'b0;
            end
        end
    end

    //=========================================================================
    // Pin Output (raw data to io_bank)
    //=========================================================================
    // Pin type processing is handled by io_bank for proper pulse timing.
    // This module just outputs the raw vector data.

    assign pin_dout = current_dout;
    assign pin_oen = current_oen;

    //=========================================================================
    // Error Detection
    //=========================================================================
    // Error detection is handled by io_bank (io_cell modules).
    // This module just passes through the error signals and generates
    // the error_valid flag.

    reg error_valid_reg;

    always @(posedge vec_clk or negedge resetn) begin
        if (!resetn) begin
            error_valid_reg <= 1'b0;
        end else begin
            // error_mask comes from io_bank via pin_din repurposed port
            // Check if any errors are present
            error_valid_reg <= vec_active && (error_mask != {VECTOR_WIDTH{1'b0}});
        end
    end

    assign error_valid = error_valid_reg;

    //=========================================================================
    // Counters
    //=========================================================================
    reg [31:0] vector_count_reg;
    reg [63:0] cycle_count_reg;

    always @(posedge vec_clk or negedge resetn) begin
        if (!resetn) begin
            vector_count_reg <= 32'd0;
            cycle_count_reg <= 64'd0;
        end else if (!enable) begin
            vector_count_reg <= 32'd0;
            cycle_count_reg <= 64'd0;
        end else begin
            // Increment vector count on new vector load
            if (load_new) begin
                vector_count_reg <= vector_count_reg + 1;
            end
            // Increment cycle count every active cycle
            if (vec_active) begin
                cycle_count_reg <= cycle_count_reg + 1;
            end
        end
    end

    assign vector_count = vector_count_reg;
    assign cycle_count = cycle_count_reg;

    //=========================================================================
    // Status
    //=========================================================================
    assign running = vec_active;
    assign done = !vec_active && repeat_done && !in_valid;

endmodule
