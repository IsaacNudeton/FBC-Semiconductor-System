`timescale 1ns / 1ps
//=============================================================================
// Error BRAM — Dual-Port Block RAM for Error Capture
//=============================================================================
//
// Port A: Write from error_counter (capture side, vec_clk domain)
// Port B: Read from firmware via AXI (query side, clk_100m domain)
//
// Parameterized width to handle:
//   - Error patterns (128-bit, VECTOR_WIDTH)
//   - Vector numbers (32-bit)
//   - Cycle counts (64-bit)
//
// Uses inferred BRAM (Vivado/Yosys will map to RAMB36E1 automatically).
//
// Isaac Oravec & Claude, March 2026
//=============================================================================

module error_bram #(
    parameter DATA_WIDTH = 32,
    parameter ADDR_WIDTH = 10,    // 1024 entries (matches MAX_ERROR_COUNT)
    parameter DEPTH      = 1024
)(
    //=========================================================================
    // Port A — Write (from error_counter)
    //=========================================================================
    input  wire                    clk_a,
    input  wire [ADDR_WIDTH-1:0]  addr_a,
    input  wire [DATA_WIDTH-1:0]  din_a,
    input  wire                   we_a,

    //=========================================================================
    // Port B — Read (from AXI/firmware)
    //=========================================================================
    input  wire                    clk_b,
    input  wire [ADDR_WIDTH-1:0]  addr_b,
    output reg  [DATA_WIDTH-1:0]  dout_b
);

    // Inferred dual-port BRAM
    (* ram_style = "block" *)
    reg [DATA_WIDTH-1:0] mem [0:DEPTH-1];

    // Port A: write-only
    always @(posedge clk_a) begin
        if (we_a) begin
            mem[addr_a] <= din_a;
        end
    end

    // Port B: read-only (1-cycle latency)
    always @(posedge clk_b) begin
        dout_b <= mem[addr_b];
    end

endmodule
