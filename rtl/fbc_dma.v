`timescale 1ns / 1ps
//=============================================================================
// FBC DMA Controller — Custom AXI DMA (no Vivado IPs)
//=============================================================================
//
// Reads FBC bytecode from DDR/OCM via S_AXI_HP0 and outputs 256-bit
// AXI-Stream words to the FBC decoder.
//
// Register interface matches Xilinx DMA register layout so existing
// firmware (dma.rs) works without changes.
//
// Register map (AXI-Lite slave on GP0 at 0x4040_0000):
//   0x00  MM2S_DMACR   — bit0=run, bit2=reset, bit12=irq_en
//   0x04  MM2S_DMASR   — bit0=halted, bit1=idle, bit12=ioc_irq, bit14=err_irq
//   0x18  MM2S_SA      — Source address (32-bit)
//   0x28  MM2S_LENGTH  — Transfer length in bytes (write starts DMA)
//
// AXI master read channel (to PS7 S_AXI_HP0, 64-bit data):
//   Bursts of 4 beats (4 x 8 bytes = 32 bytes per burst)
//   8 bursts = 256 bytes = one 256-bit AXI-Stream word
//
// Isaac Oravec & Claude, March 2026
//=============================================================================

module fbc_dma #(
    parameter AXI_ADDR_WIDTH = 12,  // Register address width (slave)
    parameter AXI_DATA_WIDTH = 32   // Register data width (slave)
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // AXI-Lite Slave (register access from PS via GP0)
    //=========================================================================
    input  wire [AXI_ADDR_WIDTH-1:0] s_axi_awaddr,
    input  wire                      s_axi_awvalid,
    output wire                      s_axi_awready,
    input  wire [AXI_DATA_WIDTH-1:0] s_axi_wdata,
    input  wire [3:0]                s_axi_wstrb,
    input  wire                      s_axi_wvalid,
    output wire                      s_axi_wready,
    output wire [1:0]                s_axi_bresp,
    output wire                      s_axi_bvalid,
    input  wire                      s_axi_bready,
    input  wire [AXI_ADDR_WIDTH-1:0] s_axi_araddr,
    input  wire                      s_axi_arvalid,
    output wire                      s_axi_arready,
    output wire [AXI_DATA_WIDTH-1:0] s_axi_rdata,
    output wire [1:0]                s_axi_rresp,
    output wire                      s_axi_rvalid,
    input  wire                      s_axi_rready,

    //=========================================================================
    // AXI Master Read (to PS7 S_AXI_HP0 — 64-bit DDR reads)
    //=========================================================================
    output reg  [31:0]               m_axi_araddr,
    output reg  [3:0]                m_axi_arlen,    // Burst length - 1
    output wire [2:0]                m_axi_arsize,   // 3 = 8 bytes per beat
    output wire [1:0]                m_axi_arburst,  // 1 = INCR
    output reg                       m_axi_arvalid,
    input  wire                      m_axi_arready,
    output wire [5:0]                m_axi_arid,
    input  wire [63:0]               m_axi_rdata,
    input  wire [1:0]                m_axi_rresp,
    input  wire                      m_axi_rvalid,
    output reg                       m_axi_rready,
    input  wire                      m_axi_rlast,
    input  wire [5:0]                m_axi_rid,

    //=========================================================================
    // AXI-Stream Master (256-bit to FBC decoder)
    //=========================================================================
    output reg  [255:0]              m_axis_tdata,
    output reg                       m_axis_tvalid,
    input  wire                      m_axis_tready,
    output reg                       m_axis_tlast,
    output wire [31:0]               m_axis_tkeep,

    //=========================================================================
    // Interrupt
    //=========================================================================
    output wire                      irq
);

    // AXI master constants
    assign m_axi_arsize  = 3'd3;     // 8 bytes per beat (64-bit HP0)
    assign m_axi_arburst = 2'd1;     // INCR
    assign m_axi_arid    = 6'd0;
    assign m_axis_tkeep  = 32'hFFFFFFFF;

    //=========================================================================
    // Registers (matching Xilinx DMA layout for firmware compatibility)
    //=========================================================================
    reg        reg_run;         // DMACR bit 0
    reg        reg_irq_en;     // DMACR bit 12
    reg        reg_halted;     // DMASR bit 0
    reg        reg_idle;       // DMASR bit 1
    reg        reg_ioc_irq;    // DMASR bit 12
    reg        reg_err_irq;    // DMASR bit 14
    reg [31:0] reg_src_addr;   // MM2S_SA
    reg [31:0] reg_length;     // MM2S_LENGTH

    // IRQ output
    assign irq = (reg_ioc_irq & reg_irq_en) | reg_err_irq;

    //=========================================================================
    // AXI-Lite Slave — Register Reads
    //=========================================================================
    reg        rd_valid;
    reg [31:0] rd_data;

    assign s_axi_arready = !rd_valid;
    assign s_axi_rdata   = rd_data;
    assign s_axi_rresp   = 2'b00;
    assign s_axi_rvalid  = rd_valid;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            rd_valid <= 1'b0;
            rd_data  <= 32'd0;
        end else begin
            if (rd_valid && s_axi_rready) begin
                rd_valid <= 1'b0;
            end
            if (s_axi_arvalid && !rd_valid) begin
                rd_valid <= 1'b1;
                case (s_axi_araddr[7:0])
                    8'h00: rd_data <= {19'b0, reg_irq_en, 9'b0, 1'b0, 1'b0, reg_run};
                    8'h04: rd_data <= {17'b0, reg_err_irq, 1'b0, reg_ioc_irq, 10'b0, reg_idle, reg_halted};
                    8'h18: rd_data <= reg_src_addr;
                    8'h28: rd_data <= reg_length;
                    default: rd_data <= 32'd0;
                endcase
            end
        end
    end

    //=========================================================================
    // AXI-Lite Slave — Register Writes
    //=========================================================================
    reg        wr_done;
    wire       wr_fire = s_axi_awvalid && s_axi_wvalid && !wr_done;
    reg        start_transfer;  // Pulse when LENGTH written
    reg        soft_reset_req;  // Pulse: DMACR bit 2
    reg        clear_ioc_req;   // Pulse: DMASR bit 12 written
    reg        clear_err_req;   // Pulse: DMASR bit 14 written

    assign s_axi_awready = wr_fire;
    assign s_axi_wready  = wr_fire;
    assign s_axi_bresp   = 2'b00;
    assign s_axi_bvalid  = wr_done;

    // AXI write handler — drives only its own signals.
    // reg_idle/reg_ioc_irq/reg_err_irq driven solely by state machine below.
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_done        <= 1'b0;
            reg_run        <= 1'b0;
            reg_irq_en     <= 1'b0;
            reg_halted     <= 1'b1;
            reg_src_addr   <= 32'd0;
            reg_length     <= 32'd0;
            start_transfer <= 1'b0;
            soft_reset_req <= 1'b0;
            clear_ioc_req  <= 1'b0;
            clear_err_req  <= 1'b0;
        end else begin
            start_transfer <= 1'b0;
            soft_reset_req <= 1'b0;
            clear_ioc_req  <= 1'b0;
            clear_err_req  <= 1'b0;

            if (wr_done && s_axi_bready) begin
                wr_done <= 1'b0;
            end

            if (wr_fire) begin
                wr_done <= 1'b1;
                case (s_axi_awaddr[7:0])
                    8'h00: begin  // DMACR
                        reg_run    <= s_axi_wdata[0];
                        reg_irq_en <= s_axi_wdata[12];
                        if (s_axi_wdata[2]) begin
                            // Soft reset
                            reg_halted     <= 1'b1;
                            soft_reset_req <= 1'b1;
                        end
                        if (s_axi_wdata[0]) begin
                            reg_halted <= 1'b0;
                        end
                    end
                    8'h04: begin  // DMASR — write-1-to-clear for IRQ bits
                        if (s_axi_wdata[12]) clear_ioc_req <= 1'b1;
                        if (s_axi_wdata[14]) clear_err_req <= 1'b1;
                    end
                    8'h18: begin  // MM2S_SA
                        reg_src_addr <= s_axi_wdata;
                    end
                    8'h1C: begin  // MM2S_SA_MSB (ignore — 32-bit addressing on Zynq)
                    end
                    8'h28: begin  // MM2S_LENGTH — starts the transfer
                        reg_length     <= s_axi_wdata;
                        start_transfer <= 1'b1;
                    end
                endcase
            end
        end
    end

    //=========================================================================
    // DMA State Machine
    //=========================================================================
    localparam S_IDLE       = 3'd0;
    localparam S_BURST_REQ  = 3'd1;
    localparam S_BURST_READ = 3'd2;
    localparam S_STREAM_OUT = 3'd3;
    localparam S_DONE       = 3'd4;

    reg [2:0]  state;
    reg [31:0] current_addr;   // Current read address
    reg [31:0] bytes_left;     // Bytes remaining
    reg [1:0]  beat_count;     // Beats collected (0-3 for 4 x 64b = 256b)
    reg [255:0] pack_reg;      // Accumulates 4 x 64-bit beats

    // Each burst: ARLEN=3 → 4 beats × 8 bytes = 32 bytes
    // One 256-bit AXI-Stream word = 32 bytes = 1 burst
    localparam BURST_LEN   = 4'd3;   // 4 beats (ARLEN = len - 1)
    localparam BURST_BYTES = 32;     // 4 × 8 bytes

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            state        <= S_IDLE;
            current_addr <= 32'd0;
            bytes_left   <= 32'd0;
            beat_count   <= 2'd0;
            pack_reg     <= 256'd0;
            m_axi_araddr  <= 32'd0;
            m_axi_arlen   <= 4'd0;
            m_axi_arvalid <= 1'b0;
            m_axi_rready  <= 1'b0;
            m_axis_tdata  <= 256'd0;
            m_axis_tvalid <= 1'b0;
            m_axis_tlast  <= 1'b0;
            reg_idle      <= 1'b1;
            reg_ioc_irq   <= 1'b0;
            reg_err_irq   <= 1'b0;
        end else begin
            // Handle AXI write requests for status registers
            if (soft_reset_req) begin
                reg_idle    <= 1'b1;
                reg_ioc_irq <= 1'b0;
                reg_err_irq <= 1'b0;
            end
            if (clear_ioc_req) reg_ioc_irq <= 1'b0;
            if (clear_err_req) reg_err_irq <= 1'b0;

            case (state)
                // ---------------------------------------------------------
                S_IDLE: begin
                    m_axi_arvalid <= 1'b0;
                    m_axi_rready  <= 1'b0;
                    m_axis_tvalid <= 1'b0;
                    m_axis_tlast  <= 1'b0;

                    if (start_transfer && reg_run) begin
                        current_addr <= reg_src_addr;
                        // Align length up to 32-byte boundary
                        bytes_left   <= (reg_length + 31) & 32'hFFFF_FFE0;
                        reg_idle     <= 1'b0;
                        state        <= S_BURST_REQ;
                    end
                end

                // ---------------------------------------------------------
                S_BURST_REQ: begin
                    if (bytes_left == 0) begin
                        // Transfer complete
                        state <= S_DONE;
                    end else begin
                        // Issue read burst: 4 beats x 8 bytes = 32 bytes
                        m_axi_araddr  <= current_addr;
                        m_axi_arlen   <= BURST_LEN;
                        m_axi_arvalid <= 1'b1;
                        m_axi_rready  <= 1'b0;
                        beat_count    <= 2'd0;

                        if (m_axi_arvalid && m_axi_arready) begin
                            m_axi_arvalid <= 1'b0;
                            m_axi_rready  <= 1'b1;
                            state         <= S_BURST_READ;
                        end
                    end
                end

                // ---------------------------------------------------------
                S_BURST_READ: begin
                    if (m_axi_rvalid && m_axi_rready) begin
                        // Pack 64-bit beat into 256-bit register
                        case (beat_count)
                            2'd0: pack_reg[63:0]    <= m_axi_rdata;
                            2'd1: pack_reg[127:64]  <= m_axi_rdata;
                            2'd2: pack_reg[191:128] <= m_axi_rdata;
                            2'd3: pack_reg[255:192] <= m_axi_rdata;
                        endcase

                        if (beat_count == 2'd3 || m_axi_rlast) begin
                            // All 4 beats collected — push to stream
                            m_axi_rready <= 1'b0;
                            state        <= S_STREAM_OUT;
                        end else begin
                            beat_count <= beat_count + 1;
                        end
                    end
                end

                // ---------------------------------------------------------
                // Present the packed 256-bit word on AXI-Stream and wait
                // for downstream (fbc_decoder) to accept it.
                //
                // IMPORTANT: address and byte count must only update on
                // the handshake beat (tvalid && tready), not every cycle.
                // Otherwise backpressure from the decoder corrupts the
                // transfer by decrementing bytes_left multiple times.
                // ---------------------------------------------------------
                S_STREAM_OUT: begin
                    m_axis_tdata  <= pack_reg;
                    m_axis_tvalid <= 1'b1;
                    m_axis_tlast  <= (bytes_left <= BURST_BYTES);

                    if (m_axis_tvalid && m_axis_tready) begin
                        // Handshake complete — advance to next burst or finish
                        m_axis_tvalid <= 1'b0;
                        m_axis_tlast  <= 1'b0;
                        current_addr  <= current_addr + BURST_BYTES;

                        if (bytes_left <= BURST_BYTES) begin
                            bytes_left <= 32'd0;
                            state      <= S_DONE;
                        end else begin
                            bytes_left <= bytes_left - BURST_BYTES;
                            state      <= S_BURST_REQ;
                        end
                    end
                end

                // ---------------------------------------------------------
                S_DONE: begin
                    m_axis_tvalid <= 1'b0;
                    m_axis_tlast  <= 1'b0;
                    reg_idle      <= 1'b1;
                    reg_ioc_irq   <= 1'b1;  // Interrupt on complete
                    state         <= S_IDLE;
                end

                default: state <= S_IDLE;
            endcase
        end
    end

endmodule
