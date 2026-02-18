`timescale 1ns / 1ps
//=============================================================================
// Error Counter - Error Statistics and Logging
//=============================================================================
//
// Tracks errors from vector engine and provides:
// - Total error count
// - First error vector/cycle
// - Error rate limiting (prevents memory exhaustion)
//
//=============================================================================

`include "fbc_pkg.vh"

module error_counter #(
    parameter VECTOR_WIDTH = `VECTOR_WIDTH,
    parameter MAX_ERRORS = `MAX_ERROR_COUNT,
    parameter ERROR_COUNT_WIDTH = 32
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // Error Input (from vector_engine)
    //=========================================================================
    input wire [VECTOR_WIDTH-1:0] error_mask,    // Which pins have errors
    input wire                    error_valid,   // Error detected
    input wire [31:0]             vector_count,  // Vector number
    input wire [63:0]             cycle_count,   // Cycle number

    //=========================================================================
    // Error BRAM Interface (write port)
    //=========================================================================
    output reg [31:0]             bram_addr,     // Error buffer address
    output reg [VECTOR_WIDTH-1:0] bram_data,     // Error pattern
    output reg                    bram_we,       // Write enable

    // Vector/cycle BRAM for error correlation
    output reg [31:0]             vec_bram_addr,
    output reg [31:0]             vec_bram_data, // Vector count at error
    output reg                    vec_bram_we,

    output reg [31:0]             cyc_bram_addr,
    output reg [63:0]             cyc_bram_data, // Cycle count at error
    output reg                    cyc_bram_we,

    //=========================================================================
    // Status (readable via AXI)
    //=========================================================================
    output reg [ERROR_COUNT_WIDTH-1:0] total_error_count,
    output reg [31:0]                  first_error_vector,
    output reg [63:0]                  first_error_cycle,
    output reg                         first_error_detected,
    output reg                         error_overflow        // Hit max errors
);

    //=========================================================================
    // Error counting
    //=========================================================================
    reg [ERROR_COUNT_WIDTH-1:0] error_idx;
    wire error_full = (error_idx >= MAX_ERRORS);

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            error_idx <= 0;
            total_error_count <= 0;
            first_error_vector <= 32'hFFFFFFFF;
            first_error_cycle <= 64'hFFFFFFFFFFFFFFFF;
            first_error_detected <= 1'b0;
            error_overflow <= 1'b0;

            bram_addr <= 0;
            bram_data <= 0;
            bram_we <= 1'b0;

            vec_bram_addr <= 0;
            vec_bram_data <= 0;
            vec_bram_we <= 1'b0;

            cyc_bram_addr <= 0;
            cyc_bram_data <= 0;
            cyc_bram_we <= 1'b0;
        end else begin
            // Default: no write
            bram_we <= 1'b0;
            vec_bram_we <= 1'b0;
            cyc_bram_we <= 1'b0;

            if (error_valid) begin
                // Always count errors
                total_error_count <= total_error_count + 1;

                // Record first error
                if (!first_error_detected) begin
                    first_error_detected <= 1'b1;
                    first_error_vector <= vector_count;
                    first_error_cycle <= cycle_count;
                end

                // Log to BRAM if not full
                if (!error_full) begin
                    // Error pattern BRAM
                    bram_addr <= {error_idx[ERROR_COUNT_WIDTH-5:0], 4'b0000};  // 16-byte aligned
                    bram_data <= error_mask;
                    bram_we <= 1'b1;

                    // Vector number BRAM
                    vec_bram_addr <= {error_idx[ERROR_COUNT_WIDTH-3:0], 2'b00};  // 4-byte aligned
                    vec_bram_data <= vector_count;
                    vec_bram_we <= 1'b1;

                    // Cycle number BRAM
                    cyc_bram_addr <= {error_idx[ERROR_COUNT_WIDTH-4:0], 3'b000};  // 8-byte aligned
                    cyc_bram_data <= cycle_count;
                    cyc_bram_we <= 1'b1;

                    error_idx <= error_idx + 1;
                end else begin
                    error_overflow <= 1'b1;
                end
            end
        end
    end

endmodule
