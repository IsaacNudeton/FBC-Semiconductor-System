`timescale 1ns / 1ps
//=============================================================================
// AXI Stream to FBC Interface
//=============================================================================
//
// Converts AXI4-Stream input from DMA to FBC instruction format.
//
// Input format (from DMA):
//   - 256-bit tdata: [63:0] instruction, [191:64] payload (128-bit), [255:192] reserved
//   - tlast: End of program marker
//
// Output format (to FBC decoder):
//   - 64-bit instruction word
//   - 128-bit payload
//
//=============================================================================

`include "fbc_pkg.vh"

module axi_stream_fbc #(
    parameter AXIS_DATA_WIDTH = 256
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // AXI4-Stream Slave (from DMA)
    //=========================================================================
    input wire [AXIS_DATA_WIDTH-1:0] s_axis_tdata,
    input wire                       s_axis_tvalid,
    output wire                      s_axis_tready,
    input wire                       s_axis_tlast,
    input wire [AXIS_DATA_WIDTH/8-1:0] s_axis_tkeep,

    //=========================================================================
    // FBC Output (to decoder)
    //=========================================================================
    output wire [63:0]  fbc_instr,
    output wire [127:0] fbc_payload,
    output wire         fbc_valid,
    input wire          fbc_ready,
    output wire         fbc_last,      // Last instruction in stream

    //=========================================================================
    // Status
    //=========================================================================
    output reg [31:0]   instr_received,
    output reg          stream_done
);

    //=========================================================================
    // Data extraction
    //=========================================================================
    // AXI Stream data format:
    // [63:0]    = FBC instruction (opcode + flags + operand)
    // [191:64]  = 128-bit payload (for SET_PINS, etc.)
    // [255:192] = Reserved / future use

    assign fbc_instr   = s_axis_tdata[63:0];
    assign fbc_payload = s_axis_tdata[191:64];
    assign fbc_valid   = s_axis_tvalid;
    assign s_axis_tready = fbc_ready;
    assign fbc_last    = s_axis_tlast;

    //=========================================================================
    // Statistics
    //=========================================================================
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            instr_received <= 32'd0;
            stream_done <= 1'b0;
        end else begin
            if (s_axis_tvalid && s_axis_tready) begin
                instr_received <= instr_received + 1;

                if (s_axis_tlast) begin
                    stream_done <= 1'b1;
                end
            end
        end
    end

endmodule
