`timescale 1ns / 1ps
//=============================================================================
// AXI4-Lite Vector Status Interface
//=============================================================================
//
// Read-only register interface for vector execution status.
// Exposes error_counter and vector_engine signals to ARM via AXI.
//
// Register Map (active at 0x4006_0000):
//   0x00: ERROR_COUNT   - Total errors detected (RO)
//   0x04: VECTOR_COUNT  - Vectors executed (RO)
//   0x08: CYCLE_LO      - Cycle count low 32 bits (RO)
//   0x0C: CYCLE_HI      - Cycle count high 32 bits (RO)
//   0x10: FIRST_ERR_VEC - First error vector number (RO)
//   0x14: STATUS        - Status bits [1:0] = {has_errors, done} (RO)
//   0x18: FIRST_ERR_LO  - First error cycle low 32 bits (RO)
//   0x1C: FIRST_ERR_HI  - First error cycle high 32 bits (RO)
//   0x3C: VERSION       - Version register (RO)
//
//=============================================================================

`include "fbc_pkg.vh"

module axi_vector_status #(
    parameter AXI_ADDR_WIDTH = 12,
    parameter AXI_DATA_WIDTH = 32
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // AXI4-Lite Slave Interface
    //=========================================================================
    // Write address (directly connect, responds OKAY but ignores)
    input wire [AXI_ADDR_WIDTH-1:0] s_axi_awaddr,
    input wire                       s_axi_awvalid,
    output reg                       s_axi_awready,

    // Write data
    input wire [AXI_DATA_WIDTH-1:0] s_axi_wdata,
    input wire [3:0]                s_axi_wstrb,
    input wire                      s_axi_wvalid,
    output reg                      s_axi_wready,

    // Write response
    output reg [1:0]                s_axi_bresp,
    output reg                      s_axi_bvalid,
    input wire                      s_axi_bready,

    // Read address
    input wire [AXI_ADDR_WIDTH-1:0] s_axi_araddr,
    input wire                      s_axi_arvalid,
    output reg                      s_axi_arready,

    // Read data
    output reg [AXI_DATA_WIDTH-1:0] s_axi_rdata,
    output reg [1:0]                s_axi_rresp,
    output reg                      s_axi_rvalid,
    input wire                      s_axi_rready,

    //=========================================================================
    // Status Inputs (from error_counter and vector_engine)
    //=========================================================================
    input wire [31:0] error_count,        // Total errors detected
    input wire [31:0] vector_count,       // Vectors executed
    input wire [63:0] cycle_count,        // Cycles executed
    input wire [31:0] first_error_vector, // Vector number of first error
    input wire [63:0] first_error_cycle,  // Cycle number of first error
    input wire        first_error_valid,  // First error has been captured
    input wire        done,               // Test complete
    input wire        has_errors          // Any errors detected
);

    //=========================================================================
    // Register Offsets
    //=========================================================================
    localparam REG_ERROR_COUNT   = 6'h00;  // 0x00
    localparam REG_VECTOR_COUNT  = 6'h01;  // 0x04
    localparam REG_CYCLE_LO      = 6'h02;  // 0x08
    localparam REG_CYCLE_HI      = 6'h03;  // 0x0C
    localparam REG_FIRST_ERR_VEC = 6'h04;  // 0x10
    localparam REG_STATUS        = 6'h05;  // 0x14
    localparam REG_FIRST_ERR_LO  = 6'h06;  // 0x18
    localparam REG_FIRST_ERR_HI  = 6'h07;  // 0x1C
    localparam REG_VERSION       = 6'h0F;  // 0x3C

    //=========================================================================
    // Write Channel (Read-Only peripheral - accept and ignore writes)
    //=========================================================================
    // Simple handshake: accept write, return OKAY, do nothing

    localparam WR_IDLE = 2'b00;
    localparam WR_DATA = 2'b01;
    localparam WR_RESP = 2'b10;

    reg [1:0] wr_state;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_state <= WR_IDLE;
            s_axi_awready <= 1'b0;
            s_axi_wready <= 1'b0;
            s_axi_bvalid <= 1'b0;
            s_axi_bresp <= 2'b00;
        end else begin
            case (wr_state)
                WR_IDLE: begin
                    s_axi_bvalid <= 1'b0;
                    if (s_axi_awvalid && s_axi_wvalid) begin
                        // Address and data arrive together
                        s_axi_awready <= 1'b1;
                        s_axi_wready <= 1'b1;
                        wr_state <= WR_RESP;
                    end else if (s_axi_awvalid) begin
                        // Address arrives first
                        s_axi_awready <= 1'b1;
                        wr_state <= WR_DATA;
                    end
                end

                WR_DATA: begin
                    s_axi_awready <= 1'b0;
                    if (s_axi_wvalid) begin
                        s_axi_wready <= 1'b1;
                        wr_state <= WR_RESP;
                    end
                end

                WR_RESP: begin
                    s_axi_awready <= 1'b0;
                    s_axi_wready <= 1'b0;
                    s_axi_bvalid <= 1'b1;
                    s_axi_bresp <= 2'b00;  // OKAY (writes are ignored but accepted)
                    if (s_axi_bready) begin
                        s_axi_bvalid <= 1'b0;
                        wr_state <= WR_IDLE;
                    end
                end

                default: wr_state <= WR_IDLE;
            endcase
        end
    end

    //=========================================================================
    // Read Channel (The actual functionality)
    //=========================================================================
    localparam RD_IDLE = 2'b00;
    localparam RD_DATA = 2'b01;

    reg [1:0] rd_state;
    reg [AXI_ADDR_WIDTH-1:0] rd_addr;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            rd_state <= RD_IDLE;
            rd_addr <= {AXI_ADDR_WIDTH{1'b0}};
            s_axi_arready <= 1'b0;
            s_axi_rvalid <= 1'b0;
            s_axi_rdata <= 32'h0;
            s_axi_rresp <= 2'b00;
        end else begin
            case (rd_state)
                RD_IDLE: begin
                    s_axi_rvalid <= 1'b0;
                    if (s_axi_arvalid) begin
                        s_axi_arready <= 1'b1;
                        rd_addr <= s_axi_araddr;
                        rd_state <= RD_DATA;
                    end
                end

                RD_DATA: begin
                    s_axi_arready <= 1'b0;
                    s_axi_rvalid <= 1'b1;
                    s_axi_rresp <= 2'b00;  // OKAY

                    // MUX: Select data based on address
                    // Address bits [7:2] give register index (word-aligned)
                    case (rd_addr[7:2])
                        REG_ERROR_COUNT:   s_axi_rdata <= error_count;
                        REG_VECTOR_COUNT:  s_axi_rdata <= vector_count;
                        REG_CYCLE_LO:      s_axi_rdata <= cycle_count[31:0];
                        REG_CYCLE_HI:      s_axi_rdata <= cycle_count[63:32];
                        REG_FIRST_ERR_VEC: s_axi_rdata <= first_error_vector;
                        REG_STATUS:        s_axi_rdata <= {29'd0, first_error_valid, has_errors, done};
                        REG_FIRST_ERR_LO:  s_axi_rdata <= first_error_cycle[31:0];
                        REG_FIRST_ERR_HI:  s_axi_rdata <= first_error_cycle[63:32];
                        REG_VERSION:       s_axi_rdata <= `FBC_VERSION;
                        default:           s_axi_rdata <= 32'hDEAD_BEEF;
                    endcase

                    if (s_axi_rready && s_axi_rvalid) begin
                        s_axi_rvalid <= 1'b0;
                        rd_state <= RD_IDLE;
                    end
                end

                default: rd_state <= RD_IDLE;
            endcase
        end
    end

endmodule
