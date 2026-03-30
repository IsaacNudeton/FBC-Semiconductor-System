`timescale 1ns / 1ps
//=============================================================================
// Clock Control - AXI Register Interface
//=============================================================================
//
// Simple AXI-Lite interface for clock frequency selection.
//
// Register Map (base 0x4008_0000):
//   0x00: FREQ_SEL  [2:0] - Frequency selection (RW)
//                          0=5MHz, 1=10MHz, 2=25MHz, 3=50MHz, 4=100MHz
//   0x04: STATUS    [0]   - MMCM locked (RO)
//   0x08: ENABLE    [0]   - vec_clk enable (RW)
//
//=============================================================================

module clk_ctrl #(
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
    input wire [AXI_ADDR_WIDTH-1:0] s_axi_awaddr,
    input wire                      s_axi_awvalid,
    output reg                      s_axi_awready,

    input wire [AXI_DATA_WIDTH-1:0] s_axi_wdata,
    input wire [3:0]                s_axi_wstrb,
    input wire                      s_axi_wvalid,
    output reg                      s_axi_wready,

    output reg [1:0]                s_axi_bresp,
    output reg                      s_axi_bvalid,
    input wire                      s_axi_bready,

    input wire [AXI_ADDR_WIDTH-1:0] s_axi_araddr,
    input wire                      s_axi_arvalid,
    output reg                      s_axi_arready,

    output reg [AXI_DATA_WIDTH-1:0] s_axi_rdata,
    output reg [1:0]                s_axi_rresp,
    output reg                      s_axi_rvalid,
    input wire                      s_axi_rready,

    //=========================================================================
    // Clock Generator Interface
    //=========================================================================
    output reg [2:0]                freq_sel,       // To clk_gen
    output reg                      vec_clk_en,     // To clk_gen
    input wire                      mmcm_locked,    // From clk_gen
    output wire                     bram_gate_n     // Active-low: 0 = BRAMs disabled during clock switch
);

    //=========================================================================
    // Register Offsets
    //=========================================================================
    localparam REG_FREQ_SEL = 4'h0;   // 0x00
    localparam REG_STATUS   = 4'h1;   // 0x04
    localparam REG_ENABLE   = 4'h2;   // 0x08

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
    // Write FSM
    //=========================================================================


    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_state <= WR_IDLE;
            wr_addr <= 0;
            s_axi_awready <= 1'b0;
            s_axi_wready <= 1'b0;
            s_axi_bvalid <= 1'b0;
            s_axi_bresp <= 2'b00;
            freq_sel <= 3'd3;    // Default: 50 MHz
            vec_clk_en <= 1'b1;  // Default: enabled (vec_clk runs at 50MHz from reset)
        end else begin
            case (wr_state)
                WR_IDLE: begin
                    s_axi_bvalid <= 1'b0;
                    if (s_axi_awvalid && s_axi_wvalid) begin
                        wr_addr <= s_axi_awaddr;
                        s_axi_awready <= 1'b1;
                        s_axi_wready <= 1'b1;
                        // Write data
                        case (s_axi_awaddr[3:2])
                            REG_FREQ_SEL: freq_sel <= s_axi_wdata[2:0];
                            REG_ENABLE:   vec_clk_en <= s_axi_wdata[0];
                            default: ;  // REG_STATUS is read-only, ignore writes
                        endcase
                        wr_state <= WR_RESP;
                    end else if (s_axi_awvalid) begin
                        wr_addr <= s_axi_awaddr;
                        s_axi_awready <= 1'b1;
                        wr_state <= WR_DATA;
                    end
                end

                WR_DATA: begin
                    s_axi_awready <= 1'b0;
                    if (s_axi_wvalid) begin
                        s_axi_wready <= 1'b1;
                        case (wr_addr[3:2])
                            REG_FREQ_SEL: freq_sel <= s_axi_wdata[2:0];
                            REG_ENABLE:   vec_clk_en <= s_axi_wdata[0];
                            default: ;  // REG_STATUS is read-only, ignore writes
                        endcase
                        wr_state <= WR_RESP;
                    end
                end

                WR_RESP: begin
                    s_axi_awready <= 1'b0;
                    s_axi_wready <= 1'b0;
                    s_axi_bvalid <= 1'b1;
                    s_axi_bresp <= 2'b00;
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
    // Clock Switch Sequencer — gates error BRAM ENA during BUFGMUX transition
    // Without this, the glitch on vec_clk during BUFGMUX switchover corrupts
    // the BRAM port A state, hanging the AXI read port and crashing the ARM.
    //=========================================================================
    reg [3:0] switch_count;  // Counts down from 15 to 0 after freq_sel change
    reg [2:0] freq_sel_prev; // Previous freq_sel to detect changes

    assign bram_gate_n = (switch_count == 0); // 1 = normal, 0 = BRAMs gated

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            switch_count <= 0;
            freq_sel_prev <= 3'd3;
        end else begin
            freq_sel_prev <= freq_sel;
            if (freq_sel != freq_sel_prev) begin
                // freq_sel just changed — start gating BRAMs
                switch_count <= 4'd15;
            end else if (switch_count != 0) begin
                switch_count <= switch_count - 1;
            end
        end
    end

    //=========================================================================
    // Read FSM
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
                        case (s_axi_araddr[3:2])
                            REG_FREQ_SEL: s_axi_rdata <= {29'b0, freq_sel};
                            REG_STATUS:   s_axi_rdata <= {31'b0, mmcm_locked};
                            REG_ENABLE:   s_axi_rdata <= {31'b0, vec_clk_en};
                            default:      s_axi_rdata <= 32'hDEAD_BEEF;
                        endcase
                        s_axi_rresp <= 2'b00;
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

endmodule
