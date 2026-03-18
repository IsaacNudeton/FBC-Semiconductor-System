`timescale 1ns / 1ps
//=============================================================================
// AXI4-Lite FBC Control Interface
//=============================================================================
//
// Register interface for FBC decoder control and status.
//
// Register Map:
//   0x00: CTRL     - Control register (R/W)
//   0x04: STATUS   - Status register (RO)
//   0x08: INSTR_LO - Instruction count low (RO)
//   0x0C: INSTR_HI - Instruction count high (RO)
//   0x10: CYCLE_LO - Cycle count low (RO)
//   0x14: CYCLE_HI - Cycle count high (RO)
//   0x18: ERROR    - Error info (RO)
//   0x1C: VERSION  - Firmware version (RO)
//   0x20: FAST_DOUT - Fast pin drive values (R/W)
//   0x24: FAST_OEN  - Fast pin output enables (R/W)
//   0x28: FAST_DIN  - Fast pin input states (RO)
//   0x2C: FAST_ERR  - Fast pin error flags (RO)
//
//=============================================================================

`include "fbc_pkg.vh"

module axi_fbc_ctrl #(
    parameter AXI_ADDR_WIDTH = 14,
    parameter AXI_DATA_WIDTH = 32
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // AXI4-Lite Slave Interface
    //=========================================================================
    // Write address
    input wire [AXI_ADDR_WIDTH-1:0] awaddr,
    input wire                       awvalid,
    output reg                       awready,

    // Write data
    input wire [AXI_DATA_WIDTH-1:0] wdata,
    input wire [3:0]                wstrb,
    input wire                      wvalid,
    output reg                      wready,

    // Write response
    output reg [1:0]                bresp,
    output reg                      bvalid,
    input wire                      bready,

    // Read address
    input wire [AXI_ADDR_WIDTH-1:0] araddr,
    input wire                      arvalid,
    output reg                      arready,

    // Read data
    output reg [AXI_DATA_WIDTH-1:0] rdata,
    output reg [1:0]                rresp,
    output reg                      rvalid,
    input wire                      rready,

    //=========================================================================
    // FBC Decoder Interface
    //=========================================================================
    output reg        fbc_enable,       // Enable decoder
    output reg        fbc_reset,        // Reset decoder
    input wire        fbc_running,      // Decoder running
    input wire        fbc_done,         // Program complete
    input wire        fbc_error,        // Decode error
    input wire [31:0] fbc_instr_count,  // Instructions executed
    input wire [63:0] fbc_cycle_count,  // Cycles generated

    //=========================================================================
    // Interrupt
    //=========================================================================
    output reg        irq_done,         // Interrupt on done
    output reg        irq_error,        // Interrupt on error

    //=========================================================================
    // Fast Pins (Bank 35 - gpio[128:159])
    //=========================================================================
    output reg [31:0] fast_dout,        // Fast pin drive values
    output reg [31:0] fast_oen,         // Fast pin output enables (1=output)
    input wire [31:0] fast_din,         // Fast pin input states (active levels)
    input wire [31:0] fast_error        // Fast pin error flags (from io_bank)
);

    //=========================================================================
    // Register addresses
    //=========================================================================
    localparam REG_CTRL     = 8'h00;
    localparam REG_STATUS   = 8'h04;
    localparam REG_INSTR_LO = 8'h08;
    localparam REG_INSTR_HI = 8'h0C;
    localparam REG_CYCLE_LO = 8'h10;
    localparam REG_CYCLE_HI = 8'h14;
    localparam REG_ERROR    = 8'h18;
    localparam REG_VERSION  = 8'h1C;
    localparam REG_FAST_DOUT = 8'h20;
    localparam REG_FAST_OEN  = 8'h24;
    localparam REG_FAST_DIN  = 8'h28;
    localparam REG_FAST_ERR  = 8'h2C;

    // State Machine Definitions
    localparam WR_IDLE = 2'd0;
    localparam WR_DATA = 2'd1;
    localparam WR_RESP = 2'd2;

    localparam RD_IDLE = 2'd0;
    localparam RD_DATA = 2'd1;

    //=========================================================================
    // Control register bits
    //=========================================================================
    // CTRL[0] = enable
    // CTRL[1] = reset (self-clearing)
    // CTRL[2] = irq_enable_done
    // CTRL[3] = irq_enable_error

    reg irq_enable_done;
    reg irq_enable_error;

    reg [1:0] wr_state;
    reg [AXI_ADDR_WIDTH-1:0] wr_addr;
    reg [1:0] rd_state;
    reg [AXI_ADDR_WIDTH-1:0] rd_addr;

    //=========================================================================
    // AXI Write State Machine
    //=========================================================================


    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_state <= WR_IDLE;
            wr_addr <= 0;
            awready <= 1'b0;
            wready <= 1'b0;
            bvalid <= 1'b0;
            bresp <= 2'b00;
            fbc_enable <= 1'b0;
            fbc_reset <= 1'b0;
            irq_enable_done <= 1'b0;
            irq_enable_error <= 1'b0;
            fast_dout <= 32'd0;
            fast_oen <= 32'd0;
        end else begin
            // Self-clearing reset
            if (fbc_reset) fbc_reset <= 1'b0;

            case (wr_state)
                WR_IDLE: begin
                    awready <= 1'b1;
                    wready <= 1'b0;
                    bvalid <= 1'b0;
                    if (awvalid && awready) begin
                        wr_addr <= awaddr;
                        awready <= 1'b0;
                        wready <= 1'b1;
                        wr_state <= WR_DATA;
                    end
                end

                WR_DATA: begin
                    if (wvalid && wready) begin
                        wready <= 1'b0;

                        // Write to register
                        case (wr_addr[7:0])
                            REG_CTRL: begin
                                fbc_enable <= wdata[0];
                                fbc_reset <= wdata[1];
                                irq_enable_done <= wdata[2];
                                irq_enable_error <= wdata[3];
                            end
                            REG_FAST_DOUT: begin
                                fast_dout <= wdata;
                            end
                            REG_FAST_OEN: begin
                                fast_oen <= wdata;
                            end
                            // Other registers are read-only
                        endcase

                        bvalid <= 1'b1;
                        bresp <= 2'b00;  // OKAY
                        wr_state <= WR_RESP;
                    end
                end

                WR_RESP: begin
                    if (bready && bvalid) begin
                        bvalid <= 1'b0;
                        wr_state <= WR_IDLE;
                    end
                end
            endcase
        end
    end

    //=========================================================================
    // AXI Read State Machine
    //=========================================================================


    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            rd_state <= RD_IDLE;
            rd_addr <= 0;
            arready <= 1'b0;
            rvalid <= 1'b0;
            rdata <= 0;
            rresp <= 2'b00;
        end else begin
            case (rd_state)
                RD_IDLE: begin
                    arready <= 1'b1;
                    rvalid <= 1'b0;
                    if (arvalid && arready) begin
                        rd_addr <= araddr;
                        arready <= 1'b0;
                        rd_state <= RD_DATA;
                    end
                end

                RD_DATA: begin
                    rvalid <= 1'b1;
                    rresp <= 2'b00;  // OKAY

                    case (rd_addr[7:0])
                        REG_CTRL: begin
                            rdata <= {28'd0, irq_enable_error, irq_enable_done,
                                     fbc_reset, fbc_enable};
                        end
                        REG_STATUS: begin
                            rdata <= {29'd0, fbc_error, fbc_done, fbc_running};
                        end
                        REG_INSTR_LO: begin
                            rdata <= fbc_instr_count;
                        end
                        REG_INSTR_HI: begin
                            rdata <= 32'd0;  // Reserved for >32-bit count
                        end
                        REG_CYCLE_LO: begin
                            rdata <= fbc_cycle_count[31:0];
                        end
                        REG_CYCLE_HI: begin
                            rdata <= fbc_cycle_count[63:32];
                        end
                        REG_ERROR: begin
                            rdata <= {31'd0, fbc_error};
                        end
                        REG_VERSION: begin
                            rdata <= `FBC_VERSION;
                        end
                        REG_FAST_DOUT: begin
                            rdata <= fast_dout;
                        end
                        REG_FAST_OEN: begin
                            rdata <= fast_oen;
                        end
                        REG_FAST_DIN: begin
                            rdata <= fast_din;
                        end
                        REG_FAST_ERR: begin
                            rdata <= fast_error;
                        end
                        default: begin
                            rdata <= 32'hDEADBEEF;
                            rresp <= 2'b10;  // SLVERR
                        end
                    endcase

                    if (rready && rvalid) begin
                        rvalid <= 1'b0;
                        rd_state <= RD_IDLE;
                    end
                end
            endcase
        end
    end

    //=========================================================================
    // Interrupt generation
    //=========================================================================
    reg fbc_done_prev;
    reg fbc_error_prev;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            irq_done <= 1'b0;
            irq_error <= 1'b0;
            fbc_done_prev <= 1'b0;
            fbc_error_prev <= 1'b0;
        end else begin
            fbc_done_prev <= fbc_done;
            fbc_error_prev <= fbc_error;

            // Rising edge detection
            irq_done <= irq_enable_done && fbc_done && !fbc_done_prev;
            irq_error <= irq_enable_error && fbc_error && !fbc_error_prev;
        end
    end

endmodule
