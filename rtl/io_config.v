`timescale 1ns / 1ps
//=============================================================================
// I/O Configuration - Pin Type and Pulse Timing Registers
//=============================================================================
//
// AXI-Lite accessible registers for all 160 pins:
// - Pin type configuration (4 bits per pin, 160 pins = 640 bits = 20 regs)
// - Pulse control timing (16 bits per pin, 160 pins = 2560 bits = 80 regs)
//
// Based on 2016 reference: reference/kzhang_v2_2016/axi_io_table.v
//
// Register Map (base + offset):
//   0x000 - 0x04F:  Pin type registers (20 x 32-bit = 160 pins x 4 bits)
//   0x100 - 0x23F:  Pulse control registers (80 x 32-bit = 160 pins x 16 bits)
//
// Pins 0-127:   BIM pins (through Quad Board to DUT)
// Pins 128-159: Fast pins (direct FPGA, no BIM latency)
//
//=============================================================================

`include "fbc_pkg.vh"

module io_config #(
    parameter WIDTH = `PIN_COUNT,             // 160 total pins
    parameter BIM_WIDTH = `VECTOR_WIDTH,      // 128 BIM pins
    parameter FAST_WIDTH = `FAST_WIDTH,       // 32 fast pins
    parameter AXI_ADDR_WIDTH = 12,
    parameter AXI_DATA_WIDTH = 32
)(
    //=========================================================================
    // Clock and Reset
    //=========================================================================
    input wire                      clk,
    input wire                      resetn,

    //=========================================================================
    // AXI4-Lite Slave Interface
    //=========================================================================
    // Write address
    input wire [AXI_ADDR_WIDTH-1:0] s_axi_awaddr,
    input wire                      s_axi_awvalid,
    output reg                      s_axi_awready,

    // Write data
    input wire [AXI_DATA_WIDTH-1:0] s_axi_wdata,
    input wire [AXI_DATA_WIDTH/8-1:0] s_axi_wstrb,
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
    // Configuration Output (directly connect to io_bank)
    //=========================================================================
    output wire [4*WIDTH-1:0]       pin_type,        // 4 bits per pin
    output wire [16*WIDTH-1:0]      pulse_ctrl_bits  // 16 bits per pin
);

    //=========================================================================
    // Register Storage
    //=========================================================================

    // Pin type: 160 pins x 4 bits = 640 bits = 20 x 32-bit registers
    // Each 32-bit register holds 8 pin types
    localparam PIN_TYPE_REGS = (WIDTH * 4 + 31) / 32;  // 20
    reg [31:0] pin_type_reg [0:PIN_TYPE_REGS-1];

    // Pulse control: 160 pins x 16 bits = 2560 bits = 80 x 32-bit registers
    // Each 32-bit register holds 2 pin pulse configs
    localparam PULSE_CTRL_REGS = (WIDTH * 16 + 31) / 32;  // 80
    reg [31:0] pulse_ctrl_reg [0:PULSE_CTRL_REGS-1];

    //=========================================================================
    // Address Decoding
    //=========================================================================
    // Bits [11:9] select region, bits [8:2] select register, bits [1:0] are byte lane
    // Region 0: Pin type (20 regs @ 0x000-0x04F)
    // Region 1-2: Pulse ctrl (80 regs @ 0x200-0x33F)
    wire [2:0]  region = s_axi_awaddr[11:9];
    wire [6:0]  reg_idx = s_axi_awaddr[8:2];

    wire [2:0]  rd_region = s_axi_araddr[11:9];
    wire [6:0]  rd_reg_idx = s_axi_araddr[8:2];

    localparam REGION_PIN_TYPE   = 3'h0;  // 0x000 - 0x1FF (only 20 regs used)
    localparam REGION_PULSE_CTRL = 3'h1;  // 0x200 - 0x3FF (80 regs used)

    // State Machine Definitions
    localparam WR_IDLE = 2'b00;
    localparam WR_DATA = 2'b01;
    localparam WR_RESP = 2'b10;

    localparam RD_IDLE = 2'b00;
    localparam RD_DATA = 2'b01;

    reg [1:0] wr_state;
    reg [AXI_ADDR_WIDTH-1:0] wr_addr;
    reg [1:0] rd_state;

    //=========================================================================
    // AXI Write FSM
    //=========================================================================


    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_state <= WR_IDLE;
            wr_addr <= {AXI_ADDR_WIDTH{1'b0}};
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
                        wr_addr <= s_axi_awaddr;
                        s_axi_awready <= 1'b1;
                        s_axi_wready <= 1'b1;
                        wr_state <= WR_RESP;
                    end else if (s_axi_awvalid) begin
                        // Address arrives first
                        wr_addr <= s_axi_awaddr;
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
                    s_axi_bresp <= 2'b00;  // OKAY
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
    // Register Write Logic
    //=========================================================================
    wire wr_en = (wr_state == WR_DATA && s_axi_wvalid) ||
                 (wr_state == WR_IDLE && s_axi_awvalid && s_axi_wvalid);

    wire [2:0]  wr_region = wr_addr[11:9];
    wire [6:0]  wr_reg_idx = wr_addr[8:2];

    integer j;
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            // Initialize all pins to BIDI (0x0)
            for (j = 0; j < PIN_TYPE_REGS; j = j + 1) begin
                pin_type_reg[j] <= 32'h0;
            end
            // Initialize pulse timing to default (start=0, end=128)
            for (j = 0; j < PULSE_CTRL_REGS; j = j + 1) begin
                pulse_ctrl_reg[j] <= 32'h0080_0080;  // [15:8]=0, [7:0]=128 for both pins
            end
        end else if (wr_en) begin
            case (wr_region)
                REGION_PIN_TYPE: begin
                    if (wr_reg_idx < PIN_TYPE_REGS) begin
                        // Byte-enable write
                        if (s_axi_wstrb[0]) pin_type_reg[wr_reg_idx][7:0]   <= s_axi_wdata[7:0];
                        if (s_axi_wstrb[1]) pin_type_reg[wr_reg_idx][15:8]  <= s_axi_wdata[15:8];
                        if (s_axi_wstrb[2]) pin_type_reg[wr_reg_idx][23:16] <= s_axi_wdata[23:16];
                        if (s_axi_wstrb[3]) pin_type_reg[wr_reg_idx][31:24] <= s_axi_wdata[31:24];
                    end
                end

                REGION_PULSE_CTRL: begin
                    if (wr_reg_idx < PULSE_CTRL_REGS) begin
                        if (s_axi_wstrb[0]) pulse_ctrl_reg[wr_reg_idx][7:0]   <= s_axi_wdata[7:0];
                        if (s_axi_wstrb[1]) pulse_ctrl_reg[wr_reg_idx][15:8]  <= s_axi_wdata[15:8];
                        if (s_axi_wstrb[2]) pulse_ctrl_reg[wr_reg_idx][23:16] <= s_axi_wdata[23:16];
                        if (s_axi_wstrb[3]) pulse_ctrl_reg[wr_reg_idx][31:24] <= s_axi_wdata[31:24];
                    end
                end

                default: wr_state <= WR_IDLE;
            endcase
        end
    end

    //=========================================================================
    // AXI Read FSM
    //=========================================================================


    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            rd_state <= RD_IDLE;
            s_axi_arready <= 1'b0;
            s_axi_rvalid <= 1'b0;
            s_axi_rdata <= 32'h0;
            s_axi_rresp <= 2'b00;
        end else begin
            case (rd_state)
                RD_IDLE: begin
                    if (s_axi_arvalid) begin
                        s_axi_arready <= 1'b1;

                        // Read data based on address
                        case (rd_region)
                            REGION_PIN_TYPE: begin
                                if (rd_reg_idx < PIN_TYPE_REGS)
                                    s_axi_rdata <= pin_type_reg[rd_reg_idx];
                                else
                                    s_axi_rdata <= 32'h0;
                            end

                            REGION_PULSE_CTRL: begin
                                if (rd_reg_idx < PULSE_CTRL_REGS)
                                    s_axi_rdata <= pulse_ctrl_reg[rd_reg_idx];
                                else
                                    s_axi_rdata <= 32'h0;
                            end

                            default: s_axi_rdata <= 32'hDEAD_BEEF;  // Undefined region
                        endcase

                        s_axi_rresp <= 2'b00;  // OKAY
                        rd_state <= RD_DATA;
                    end
                end

                RD_DATA: begin
                    s_axi_arready <= 1'b0;
                    s_axi_rvalid <= 1'b1;
                    if (s_axi_rready) begin
                        s_axi_rvalid <= 1'b0;
                        rd_state <= RD_IDLE;
                    end
                end

                default: rd_state <= RD_IDLE;
            endcase
        end
    end

    //=========================================================================
    // Output Mapping
    //=========================================================================
    // Flatten registers to output buses

    assign pin_type = {
        pin_type_reg[19],         pin_type_reg[18],         pin_type_reg[17],         pin_type_reg[16],
        pin_type_reg[15],         pin_type_reg[14],         pin_type_reg[13],         pin_type_reg[12],
        pin_type_reg[11],         pin_type_reg[10],         pin_type_reg[9],         pin_type_reg[8],
        pin_type_reg[7],         pin_type_reg[6],         pin_type_reg[5],         pin_type_reg[4],
        pin_type_reg[3],         pin_type_reg[2],         pin_type_reg[1],         pin_type_reg[0]
    };

    assign pulse_ctrl_bits = {
        pulse_ctrl_reg[79],         pulse_ctrl_reg[78],         pulse_ctrl_reg[77],         pulse_ctrl_reg[76],
        pulse_ctrl_reg[75],         pulse_ctrl_reg[74],         pulse_ctrl_reg[73],         pulse_ctrl_reg[72],
        pulse_ctrl_reg[71],         pulse_ctrl_reg[70],         pulse_ctrl_reg[69],         pulse_ctrl_reg[68],
        pulse_ctrl_reg[67],         pulse_ctrl_reg[66],         pulse_ctrl_reg[65],         pulse_ctrl_reg[64],
        pulse_ctrl_reg[63],         pulse_ctrl_reg[62],         pulse_ctrl_reg[61],         pulse_ctrl_reg[60],
        pulse_ctrl_reg[59],         pulse_ctrl_reg[58],         pulse_ctrl_reg[57],         pulse_ctrl_reg[56],
        pulse_ctrl_reg[55],         pulse_ctrl_reg[54],         pulse_ctrl_reg[53],         pulse_ctrl_reg[52],
        pulse_ctrl_reg[51],         pulse_ctrl_reg[50],         pulse_ctrl_reg[49],         pulse_ctrl_reg[48],
        pulse_ctrl_reg[47],         pulse_ctrl_reg[46],         pulse_ctrl_reg[45],         pulse_ctrl_reg[44],
        pulse_ctrl_reg[43],         pulse_ctrl_reg[42],         pulse_ctrl_reg[41],         pulse_ctrl_reg[40],
        pulse_ctrl_reg[39],         pulse_ctrl_reg[38],         pulse_ctrl_reg[37],         pulse_ctrl_reg[36],
        pulse_ctrl_reg[35],         pulse_ctrl_reg[34],         pulse_ctrl_reg[33],         pulse_ctrl_reg[32],
        pulse_ctrl_reg[31],         pulse_ctrl_reg[30],         pulse_ctrl_reg[29],         pulse_ctrl_reg[28],
        pulse_ctrl_reg[27],         pulse_ctrl_reg[26],         pulse_ctrl_reg[25],         pulse_ctrl_reg[24],
        pulse_ctrl_reg[23],         pulse_ctrl_reg[22],         pulse_ctrl_reg[21],         pulse_ctrl_reg[20],
        pulse_ctrl_reg[19],         pulse_ctrl_reg[18],         pulse_ctrl_reg[17],         pulse_ctrl_reg[16],
        pulse_ctrl_reg[15],         pulse_ctrl_reg[14],         pulse_ctrl_reg[13],         pulse_ctrl_reg[12],
        pulse_ctrl_reg[11],         pulse_ctrl_reg[10],         pulse_ctrl_reg[9],         pulse_ctrl_reg[8],
        pulse_ctrl_reg[7],         pulse_ctrl_reg[6],         pulse_ctrl_reg[5],         pulse_ctrl_reg[4],
        pulse_ctrl_reg[3],         pulse_ctrl_reg[2],         pulse_ctrl_reg[1],         pulse_ctrl_reg[0]
    };

endmodule
