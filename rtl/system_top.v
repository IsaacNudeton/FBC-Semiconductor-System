`timescale 1ns / 1ps
//=============================================================================
// System Top - Complete FBC Semiconductor Test System
//=============================================================================
//
// This is the top-level module that instantiates:
// - Clock generation (MMCM)
// - FBC execution core
// - Zynq PS block wrapper (for synthesis)
//
// Target: Zynq-7020 (xc7z020clg484-1)
//
//=============================================================================

`include "fbc_pkg.vh"

module system_top (
    //=========================================================================
    // DDR Pins (directly to Zynq PS — names match Vivado PS7 IP stub)
    //=========================================================================
    inout wire [14:0] DDR_Addr,
    inout wire [2:0]  DDR_BankAddr,
    inout wire        DDR_CAS_n,
    inout wire        DDR_Clk_n,
    inout wire        DDR_Clk,
    inout wire        DDR_CKE,
    inout wire        DDR_CS_n,
    inout wire [3:0]  DDR_DM,
    inout wire [31:0] DDR_DQ,
    inout wire [3:0]  DDR_DQS_n,
    inout wire [3:0]  DDR_DQS,
    inout wire        DDR_ODT,
    inout wire        DDR_RAS_n,
    inout wire        DDR_DRSTB,
    inout wire        DDR_WEB,
    inout wire        DDR_VRN,
    inout wire        DDR_VRP,

    //=========================================================================
    // Fixed IO (PS peripherals — names match Vivado PS7 IP stub)
    //=========================================================================
    inout wire [53:0] MIO,
    inout wire        PS_CLK,
    inout wire        PS_PORB,
    inout wire        PS_SRSTB,

    //=========================================================================
    // Test Vector I/O (directly to DUT interface board)
    //=========================================================================
    // gpio[0:127]   - BIM pins (through Board Interface Module)
    // gpio[128:159] - Fast pins (direct FPGA, 1-cycle latency)
    inout wire [`PIN_COUNT-1:0] gpio,       // 160 bidirectional test pins

    //=========================================================================
    // Clock Outputs (to DUT) - 4 differential pairs
    //=========================================================================
    // clk_out[0] - Variable frequency (5/10/25/50/100 MHz)
    // clk_out[1] - Fixed 100 MHz CML
    // clk_out[2] - Variable 10-25 MHz single-ended
    // clk_out[3] - Fixed 10 MHz differential
    output wire [3:0] clk_out_p,
    output wire [3:0] clk_out_n
    // No status LEDs — Sonoma board has no user LEDs on PL fabric.
    // All status readable via AXI registers (axi_fbc_ctrl, axi_vector_status).
);

    //=========================================================================
    // Internal Signals - Clocks
    //=========================================================================
    wire clk_100m;              // From PS - AXI clock
    wire clk_200m;              // From PS - High-speed clock
    wire vec_clk;               // Generated - Vector execution
    wire vec_clk_90;            // Generated - 90° phase
    wire vec_clk_180;           // Generated - 180° phase
    wire delay_clk;             // Generated - I/O timing
    wire clk_locked;            // MMCM locked

    //=========================================================================
    // Internal Signals - Reset
    //=========================================================================
    wire ps_resetn;             // From PS
    wire sys_resetn;            // Synchronized reset

    // Synchronize reset to clk_100m domain
    reg [2:0] reset_sync;
    always @(posedge clk_100m or negedge ps_resetn) begin
        if (!ps_resetn)
            reset_sync <= 3'b000;
        else
            reset_sync <= {reset_sync[1:0], clk_locked};
    end
    assign sys_resetn = reset_sync[2];

    //=========================================================================
    // Internal Signals - AXI Stream (DMA to FBC)
    //=========================================================================
    wire [255:0] axis_tdata;
    wire         axis_tvalid;
    wire         axis_tready;
    wire         axis_tlast;
    wire [31:0]  axis_tkeep;

    //=========================================================================
    // Internal Signals - AXI-Lite (Control registers)
    //=========================================================================
    // FBC Control
    wire [13:0] axi_fbc_awaddr, axi_fbc_araddr;
    wire        axi_fbc_awvalid, axi_fbc_awready;
    wire        axi_fbc_arvalid, axi_fbc_arready;
    wire [31:0] axi_fbc_wdata, axi_fbc_rdata;
    wire [3:0]  axi_fbc_wstrb;
    wire        axi_fbc_wvalid, axi_fbc_wready;
    wire [1:0]  axi_fbc_bresp, axi_fbc_rresp;
    wire        axi_fbc_bvalid, axi_fbc_bready;
    wire        axi_fbc_rvalid, axi_fbc_rready;

    // I/O Config
    wire [11:0] axi_io_awaddr, axi_io_araddr;
    wire        axi_io_awvalid, axi_io_awready;
    wire        axi_io_arvalid, axi_io_arready;
    wire [31:0] axi_io_wdata, axi_io_rdata;
    wire [3:0]  axi_io_wstrb;
    wire        axi_io_wvalid, axi_io_wready;
    wire [1:0]  axi_io_bresp, axi_io_rresp;
    wire        axi_io_bvalid, axi_io_bready;
    wire        axi_io_rvalid, axi_io_rready;

    //=========================================================================
    // Internal Signals - Pin Interface (all 160 pins)
    //=========================================================================
    // Combined 160-pin interface to/from fbc_top
    // [127:0]   = BIM pins (through Board Interface Module, 2-cycle latency)
    // [159:128] = Fast pins (direct FPGA, 1-cycle latency)
    wire [`PIN_COUNT-1:0] pin_dout;         // Output data to pins
    wire [`PIN_COUNT-1:0] pin_oen;          // Output enable (T=1=input, T=0=output)
    wire [`PIN_COUNT-1:0] pin_din;          // Input data from pins

    //=========================================================================
    // Internal Signals - Clock Outputs
    //=========================================================================
    wire [3:0] pll_clk;                     // 4 clock outputs from clk_gen

    //=========================================================================
    // Internal Signals - Interrupts
    //=========================================================================
    wire irq_done;
    wire irq_error;
    wire [31:0] fast_error_w;  // Fast pin error flags from io_bank

    //=========================================================================
    // Internal Signals - FBC Control
    //=========================================================================
    wire vec_clk_en;  // From clock control

    //=========================================================================
    // Internal Signals - Clock Control
    //=========================================================================
    wire [2:0] freq_sel;  // From clk_ctrl AXI interface

    // Clock Control AXI-Lite (0x4008_0000)
    wire [11:0] axi_clk_awaddr, axi_clk_araddr;
    wire        axi_clk_awvalid, axi_clk_awready;
    wire        axi_clk_arvalid, axi_clk_arready;
    wire [31:0] axi_clk_wdata, axi_clk_rdata;
    wire [3:0]  axi_clk_wstrb;
    wire        axi_clk_wvalid, axi_clk_wready;
    wire [1:0]  axi_clk_bresp, axi_clk_rresp;
    wire        axi_clk_bvalid, axi_clk_bready;
    wire        axi_clk_rvalid, axi_clk_rready;

    // Vector Status AXI-Lite (0x4006_0000)
    wire [11:0] axi_status_awaddr, axi_status_araddr;
    wire        axi_status_awvalid, axi_status_awready;
    wire        axi_status_arvalid, axi_status_arready;
    wire [31:0] axi_status_wdata, axi_status_rdata;
    wire [3:0]  axi_status_wstrb;
    wire        axi_status_wvalid, axi_status_wready;
    wire [1:0]  axi_status_bresp, axi_status_rresp;
    wire        axi_status_bvalid, axi_status_bready;
    wire        axi_status_rvalid, axi_status_rready;

    // Frequency Counter AXI-Lite (0x4007_0000)
    wire [11:0] axi_freq_awaddr, axi_freq_araddr;
    wire        axi_freq_awvalid, axi_freq_awready;
    wire        axi_freq_arvalid, axi_freq_arready;
    wire [31:0] axi_freq_wdata, axi_freq_rdata;
    wire [3:0]  axi_freq_wstrb;
    wire        axi_freq_wvalid, axi_freq_wready;
    wire [1:0]  axi_freq_bresp, axi_freq_rresp;
    wire        axi_freq_bvalid, axi_freq_bready;
    wire        axi_freq_rvalid, axi_freq_rready;
    wire        irq_freq;  // Frequency counter interrupt

    //=========================================================================
    // Clock Generation (ONETWO: Pre-gen MUX approach)
    //=========================================================================
    // INVARIANT: 5 frequencies pre-generated (5/10/25/50/100 MHz)
    // VARIES: Which frequency is selected via freq_sel
    // BENEFIT: <100ns switch time vs 100µs for DRP PLL relock
    clk_gen u_clk_gen (
        .clk_100m       (clk_100m),
        .clk_200m       (clk_200m),
        .resetn         (ps_resetn),

        .vec_clk        (vec_clk),
        .vec_clk_90     (vec_clk_90),
        .vec_clk_180    (vec_clk_180),
        .delay_clk      (delay_clk),

        .vec_clk_en     (vec_clk_en),
        .locked         (clk_locked),

        .freq_sel       (freq_sel),       // 0=5M, 1=10M, 2=25M, 3=50M, 4=100M

        // Clock outputs for OBUFDS (to DUT)
        .pll_clk        (pll_clk)         // 4 clock outputs
    );

    //=========================================================================
    // Clock Control AXI Register Interface (0x4008_0000)
    //=========================================================================
    clk_ctrl #(
        .AXI_ADDR_WIDTH(12),
        .AXI_DATA_WIDTH(32)
    ) u_clk_ctrl (
        .clk            (clk_100m),
        .resetn         (sys_resetn),

        // AXI-Lite
        .s_axi_awaddr   (axi_clk_awaddr),
        .s_axi_awvalid  (axi_clk_awvalid),
        .s_axi_awready  (axi_clk_awready),
        .s_axi_wdata    (axi_clk_wdata),
        .s_axi_wstrb    (axi_clk_wstrb),
        .s_axi_wvalid   (axi_clk_wvalid),
        .s_axi_wready   (axi_clk_wready),
        .s_axi_bresp    (axi_clk_bresp),
        .s_axi_bvalid   (axi_clk_bvalid),
        .s_axi_bready   (axi_clk_bready),
        .s_axi_araddr   (axi_clk_araddr),
        .s_axi_arvalid  (axi_clk_arvalid),
        .s_axi_arready  (axi_clk_arready),
        .s_axi_rdata    (axi_clk_rdata),
        .s_axi_rresp    (axi_clk_rresp),
        .s_axi_rvalid   (axi_clk_rvalid),
        .s_axi_rready   (axi_clk_rready),

        // Clock Generator Interface
        .freq_sel       (freq_sel),
        .vec_clk_en     (vec_clk_en),
        .mmcm_locked    (clk_locked)
    );

    //=========================================================================
    // Frequency Counter (0x4007_0000) - Measures DUT signal frequencies
    //=========================================================================
    axi_freq_counter #(
        .AXI_ADDR_WIDTH(12),
        .AXI_DATA_WIDTH(32),
        .NUM_COUNTERS(4)
    ) u_freq_counter (
        .clk            (clk_100m),
        .resetn         (sys_resetn),

        // AXI-Lite
        .s_axi_awaddr   (axi_freq_awaddr),
        .s_axi_awvalid  (axi_freq_awvalid),
        .s_axi_awready  (axi_freq_awready),
        .s_axi_wdata    (axi_freq_wdata),
        .s_axi_wstrb    (axi_freq_wstrb),
        .s_axi_wvalid   (axi_freq_wvalid),
        .s_axi_wready   (axi_freq_wready),
        .s_axi_bresp    (axi_freq_bresp),
        .s_axi_bvalid   (axi_freq_bvalid),
        .s_axi_bready   (axi_freq_bready),
        .s_axi_araddr   (axi_freq_araddr),
        .s_axi_arvalid  (axi_freq_arvalid),
        .s_axi_arready  (axi_freq_arready),
        .s_axi_rdata    (axi_freq_rdata),
        .s_axi_rresp    (axi_freq_rresp),
        .s_axi_rvalid   (axi_freq_rvalid),
        .s_axi_rready   (axi_freq_rready),

        // Signal inputs (directly from pins - all 160 pins available)
        .all_inputs     (pin_din),

        // Interrupt
        .irq            (irq_freq)
    );

    //=========================================================================
    // FBC Core (execution engine)
    //=========================================================================
    fbc_top #(
        .VECTOR_WIDTH(`VECTOR_WIDTH),
        .FAST_WIDTH(`FAST_WIDTH),
        .PIN_COUNT(`PIN_COUNT),
        .AXIS_DATA_WIDTH(256)
    ) u_fbc_top (
        // Clocks
        .clk            (clk_100m),
        .vec_clk        (vec_clk),
        .delay_clk      (delay_clk),
        .resetn         (sys_resetn),

        // AXI Stream
        .s_axis_tdata   (axis_tdata),
        .s_axis_tvalid  (axis_tvalid),
        .s_axis_tready  (axis_tready),
        .s_axis_tlast   (axis_tlast),
        .s_axis_tkeep   (axis_tkeep),

        // AXI-Lite FBC Control
        .s_axi_fbc_awaddr  (axi_fbc_awaddr),
        .s_axi_fbc_awvalid (axi_fbc_awvalid),
        .s_axi_fbc_awready (axi_fbc_awready),
        .s_axi_fbc_wdata   (axi_fbc_wdata),
        .s_axi_fbc_wstrb   (axi_fbc_wstrb),
        .s_axi_fbc_wvalid  (axi_fbc_wvalid),
        .s_axi_fbc_wready  (axi_fbc_wready),
        .s_axi_fbc_bresp   (axi_fbc_bresp),
        .s_axi_fbc_bvalid  (axi_fbc_bvalid),
        .s_axi_fbc_bready  (axi_fbc_bready),
        .s_axi_fbc_araddr  (axi_fbc_araddr),
        .s_axi_fbc_arvalid (axi_fbc_arvalid),
        .s_axi_fbc_arready (axi_fbc_arready),
        .s_axi_fbc_rdata   (axi_fbc_rdata),
        .s_axi_fbc_rresp   (axi_fbc_rresp),
        .s_axi_fbc_rvalid  (axi_fbc_rvalid),
        .s_axi_fbc_rready  (axi_fbc_rready),

        // AXI-Lite I/O Config
        .s_axi_io_awaddr   (axi_io_awaddr),
        .s_axi_io_awvalid  (axi_io_awvalid),
        .s_axi_io_awready  (axi_io_awready),
        .s_axi_io_wdata    (axi_io_wdata),
        .s_axi_io_wstrb    (axi_io_wstrb),
        .s_axi_io_wvalid   (axi_io_wvalid),
        .s_axi_io_wready   (axi_io_wready),
        .s_axi_io_bresp    (axi_io_bresp),
        .s_axi_io_bvalid   (axi_io_bvalid),
        .s_axi_io_bready   (axi_io_bready),
        .s_axi_io_araddr   (axi_io_araddr),
        .s_axi_io_arvalid  (axi_io_arvalid),
        .s_axi_io_arready  (axi_io_arready),
        .s_axi_io_rdata    (axi_io_rdata),
        .s_axi_io_rresp    (axi_io_rresp),
        .s_axi_io_rvalid   (axi_io_rvalid),
        .s_axi_io_rready   (axi_io_rready),

        // AXI-Lite Vector Status
        .s_axi_status_awaddr   (axi_status_awaddr),
        .s_axi_status_awvalid  (axi_status_awvalid),
        .s_axi_status_awready  (axi_status_awready),
        .s_axi_status_wdata    (axi_status_wdata),
        .s_axi_status_wstrb    (axi_status_wstrb),
        .s_axi_status_wvalid   (axi_status_wvalid),
        .s_axi_status_wready   (axi_status_wready),
        .s_axi_status_bresp    (axi_status_bresp),
        .s_axi_status_bvalid   (axi_status_bvalid),
        .s_axi_status_bready   (axi_status_bready),
        .s_axi_status_araddr   (axi_status_araddr),
        .s_axi_status_arvalid  (axi_status_arvalid),
        .s_axi_status_arready  (axi_status_arready),
        .s_axi_status_rdata    (axi_status_rdata),
        .s_axi_status_rresp    (axi_status_rresp),
        .s_axi_status_rvalid   (axi_status_rvalid),
        .s_axi_status_rready   (axi_status_rready),

        // Pins (all 160: BIM[127:0] + Fast[159:128])
        .pin_dout       (pin_dout),
        .pin_oen        (pin_oen),
        .pin_din        (pin_din),

        // Fast pin control
        .fast_clk_en    (vec_clk_en),     // Fast pins update with vec_clk

        // Fast pin errors (routed through to axi_fbc_ctrl at 0x2C)
        .fast_error     (fast_error_w),

        // Error BRAMs (wired to error_bram instances)
        .error_bram_addr     (err_pat_addr),
        .error_bram_data     (err_pat_data),
        .error_bram_we       (err_pat_we),
        .error_vec_bram_addr (err_vec_addr),
        .error_vec_bram_data (err_vec_data),
        .error_vec_bram_we   (err_vec_we),
        .error_cyc_bram_addr (err_cyc_addr),
        .error_cyc_bram_data (err_cyc_data),
        .error_cyc_bram_we   (err_cyc_we),

        // Interrupts
        .irq_done       (irq_done),
        .irq_error      (irq_error)
    );

    //=========================================================================
    // I/O Buffers (IOBUF for all 160 bidirectional pins)
    //=========================================================================
    // gpio[0:127]   - BIM pins (2-cycle latency through Board Interface Module)
    // gpio[128:159] - Fast pins (1-cycle latency, direct FPGA routing)
    //
    // fbc_top handles the split internally:
    //   - BIM pins controlled by vector engine via bytecode
    //   - Fast pins controlled via AXI registers (axi_fbc_ctrl)
    //=========================================================================
    genvar i;
    generate
        for (i = 0; i < `PIN_COUNT; i = i + 1) begin : io_bufs
            IOBUF u_iobuf (
                .IO (gpio[i]),
                .I  (pin_dout[i]),
                .O  (pin_din[i]),
                .T  (pin_oen[i])      // T=1 = tristate (input), T=0 = output
            );
        end
    endgenerate

    //=========================================================================
    // Differential Clock Outputs (4 pairs to DUT)
    //=========================================================================
    // clk_out[0]: Variable frequency (vec_clk - 5/10/25/50/100 MHz)
    OBUFDS #(.IOSTANDARD("LVDS_25")) u_clk_out0_obuf (
        .O  (clk_out_p[0]),
        .OB (clk_out_n[0]),
        .I  (pll_clk[0])
    );

    // clk_out[1]: Fixed 100 MHz CML
    OBUFDS #(.IOSTANDARD("LVDS_25")) u_clk_out1_obuf (
        .O  (clk_out_p[1]),
        .OB (clk_out_n[1]),
        .I  (pll_clk[1])
    );

    // clk_out[2]: Variable 10-25 MHz single-ended
    OBUFDS #(.IOSTANDARD("LVDS_25")) u_clk_out2_obuf (
        .O  (clk_out_p[2]),
        .OB (clk_out_n[2]),
        .I  (pll_clk[2])
    );

    // clk_out[3]: Fixed 10 MHz differential
    OBUFDS #(.IOSTANDARD("LVDS_25")) u_clk_out3_obuf (
        .O  (clk_out_p[3]),
        .OB (clk_out_n[3]),
        .I  (pll_clk[3])
    );

    // Status available via AXI registers:
    //   clk_locked → clk_ctrl STATUS[0]
    //   vec_clk_en → clk_ctrl ENABLE[0]
    //   irq_done   → axi_fbc_ctrl STATUS[1]
    //   irq_error  → axi_fbc_ctrl STATUS[2]

    //=========================================================================
    // Zynq PS Block
    //=========================================================================
    // The Processing System 7 (PS7) provides:
    //   - FCLK_CLK0 (100 MHz) - AXI bus clock
    //   - FCLK_CLK1 (200 MHz) - High-speed clock for vector timing
    //   - ps_resetn - System reset
    //   - M_AXI_GP0 - Master AXI port for register access
    //   - S_AXI_HP0 - Slave AXI port for DMA from DDR
    //   - IRQ_F2P - Fabric-to-PS interrupts

    // AXI Master interface from PS (to AXI Interconnect)
    wire [31:0] m_axi_gp0_awaddr, m_axi_gp0_araddr;
    wire        m_axi_gp0_awvalid, m_axi_gp0_awready;
    wire        m_axi_gp0_arvalid, m_axi_gp0_arready;
    wire [31:0] m_axi_gp0_wdata, m_axi_gp0_rdata;
    wire [3:0]  m_axi_gp0_wstrb;
    wire        m_axi_gp0_wvalid, m_axi_gp0_wready;
    wire [1:0]  m_axi_gp0_bresp, m_axi_gp0_rresp;
    wire        m_axi_gp0_bvalid, m_axi_gp0_bready;
    wire        m_axi_gp0_rvalid, m_axi_gp0_rready;
    wire [11:0] m_axi_gp0_arid, m_axi_gp0_awid;
    wire [11:0] m_axi_gp0_rid, m_axi_gp0_bid;
    wire [3:0]  m_axi_gp0_arlen, m_axi_gp0_awlen;
    wire [2:0]  m_axi_gp0_arsize, m_axi_gp0_awsize;
    wire [1:0]  m_axi_gp0_arburst, m_axi_gp0_awburst;
    wire        m_axi_gp0_rlast;

    // S_AXI_HP0 — Full AXI read channel (DMA reads DDR via PS7)
    wire [31:0] s_axi_hp0_araddr;
    wire [3:0]  s_axi_hp0_arlen;
    wire [2:0]  s_axi_hp0_arsize;
    wire [1:0]  s_axi_hp0_arburst;
    wire        s_axi_hp0_arvalid, s_axi_hp0_arready;
    wire [63:0] s_axi_hp0_rdata;
    wire [1:0]  s_axi_hp0_rresp;
    wire        s_axi_hp0_rvalid, s_axi_hp0_rready;
    wire        s_axi_hp0_rlast;
    wire [5:0]  s_axi_hp0_arid, s_axi_hp0_rid;

    // DMA Controller AXI-Lite (0x4040_0000)
    wire [11:0] axi_dma_awaddr, axi_dma_araddr;
    wire        axi_dma_awvalid, axi_dma_awready;
    wire        axi_dma_arvalid, axi_dma_arready;
    wire [31:0] axi_dma_wdata, axi_dma_rdata;
    wire [3:0]  axi_dma_wstrb;
    wire        axi_dma_wvalid, axi_dma_wready;
    wire [1:0]  axi_dma_bresp, axi_dma_rresp;
    wire        axi_dma_bvalid, axi_dma_bready;
    wire        axi_dma_rvalid, axi_dma_rready;
    wire        irq_dma;  // DMA completion interrupt

    // Error BRAM AXI-Lite (0x4009_0000)
    wire [11:0] axi_err_awaddr, axi_err_araddr;
    wire        axi_err_awvalid, axi_err_awready;
    wire        axi_err_arvalid, axi_err_arready;
    wire [31:0] axi_err_wdata, axi_err_rdata;
    wire [3:0]  axi_err_wstrb;
    wire        axi_err_wvalid, axi_err_wready;
    wire [1:0]  axi_err_bresp, axi_err_rresp;
    wire        axi_err_bvalid, axi_err_bready;
    wire        axi_err_rvalid, axi_err_rready;

    // Error BRAM write ports (from error_counter inside fbc_top)
    wire [31:0]              err_pat_addr;
    wire [`VECTOR_WIDTH-1:0] err_pat_data;
    wire                     err_pat_we;
    wire [31:0]              err_vec_addr;
    wire [31:0]              err_vec_data;
    wire                     err_vec_we;
    wire [31:0]              err_cyc_addr;
    wire [63:0]              err_cyc_data;
    wire                     err_cyc_we;

    // Error BRAM read ports (firmware query side)
    wire [`VECTOR_WIDTH-1:0] err_pat_rd;
    wire [31:0]              err_vec_rd;
    wire [63:0]              err_cyc_rd;
    reg  [9:0]               err_rd_addr;  // Registered read address from AXI

`ifndef SIMULATION
    // Synthesis: Instantiate actual PS7 block
    processing_system7_0 u_ps (
        // DDR Interface (names match PS7 IP stub exactly)
        .DDR_Addr           (DDR_Addr),
        .DDR_BankAddr       (DDR_BankAddr),
        .DDR_CAS_n          (DDR_CAS_n),
        .DDR_Clk_n          (DDR_Clk_n),
        .DDR_Clk            (DDR_Clk),
        .DDR_CKE            (DDR_CKE),
        .DDR_CS_n           (DDR_CS_n),
        .DDR_DM             (DDR_DM),
        .DDR_DQ             (DDR_DQ),
        .DDR_DQS_n          (DDR_DQS_n),
        .DDR_DQS            (DDR_DQS),
        .DDR_ODT            (DDR_ODT),
        .DDR_RAS_n          (DDR_RAS_n),
        .DDR_DRSTB          (DDR_DRSTB),
        .DDR_WEB            (DDR_WEB),
        .DDR_VRN            (DDR_VRN),
        .DDR_VRP            (DDR_VRP),

        // Fixed IO
        .MIO                (MIO),
        .PS_CLK             (PS_CLK),
        .PS_PORB            (PS_PORB),
        .PS_SRSTB           (PS_SRSTB),

        // Clocks
        .FCLK_CLK0          (clk_100m),
        .FCLK_CLK1          (clk_200m),
        .FCLK_RESET0_N      (ps_resetn),

        // M_AXI_GP0 Master Interface
        .M_AXI_GP0_ACLK     (clk_100m),
        .M_AXI_GP0_AWADDR   (m_axi_gp0_awaddr),
        .M_AXI_GP0_AWVALID  (m_axi_gp0_awvalid),
        .M_AXI_GP0_AWREADY  (m_axi_gp0_awready),
        .M_AXI_GP0_AWID     (m_axi_gp0_awid),
        .M_AXI_GP0_AWLEN    (m_axi_gp0_awlen),
        .M_AXI_GP0_AWSIZE   (m_axi_gp0_awsize),
        .M_AXI_GP0_AWBURST  (m_axi_gp0_awburst),
        .M_AXI_GP0_AWLOCK   (),
        .M_AXI_GP0_AWCACHE  (),
        .M_AXI_GP0_AWPROT   (),
        .M_AXI_GP0_AWQOS    (),
        .M_AXI_GP0_WDATA    (m_axi_gp0_wdata),
        .M_AXI_GP0_WSTRB    (m_axi_gp0_wstrb),
        .M_AXI_GP0_WVALID   (m_axi_gp0_wvalid),
        .M_AXI_GP0_WREADY   (m_axi_gp0_wready),
        .M_AXI_GP0_WLAST    (),
        .M_AXI_GP0_BID      (m_axi_gp0_bid),
        .M_AXI_GP0_BRESP    (m_axi_gp0_bresp),
        .M_AXI_GP0_BVALID   (m_axi_gp0_bvalid),
        .M_AXI_GP0_BREADY   (m_axi_gp0_bready),
        .M_AXI_GP0_ARADDR   (m_axi_gp0_araddr),
        .M_AXI_GP0_ARVALID  (m_axi_gp0_arvalid),
        .M_AXI_GP0_ARREADY  (m_axi_gp0_arready),
        .M_AXI_GP0_ARID     (m_axi_gp0_arid),
        .M_AXI_GP0_ARLEN    (m_axi_gp0_arlen),
        .M_AXI_GP0_ARSIZE   (m_axi_gp0_arsize),
        .M_AXI_GP0_ARBURST  (m_axi_gp0_arburst),
        .M_AXI_GP0_ARLOCK   (),
        .M_AXI_GP0_ARCACHE  (),
        .M_AXI_GP0_ARPROT   (),
        .M_AXI_GP0_ARQOS    (),
        .M_AXI_GP0_RID      (m_axi_gp0_rid),
        .M_AXI_GP0_RDATA    (m_axi_gp0_rdata),
        .M_AXI_GP0_RRESP    (m_axi_gp0_rresp),
        .M_AXI_GP0_RVALID   (m_axi_gp0_rvalid),
        .M_AXI_GP0_RREADY   (m_axi_gp0_rready),
        .M_AXI_GP0_RLAST    (m_axi_gp0_rlast),

        // S_AXI_HP0 — DMA reads from DDR (read channel only)
        .S_AXI_HP0_ACLK     (clk_100m),
        .S_AXI_HP0_ARID      (s_axi_hp0_arid),
        .S_AXI_HP0_ARADDR    (s_axi_hp0_araddr),
        .S_AXI_HP0_ARLEN     (s_axi_hp0_arlen),
        .S_AXI_HP0_ARSIZE    (s_axi_hp0_arsize),
        .S_AXI_HP0_ARBURST   (s_axi_hp0_arburst),
        .S_AXI_HP0_ARLOCK    (2'b00),
        .S_AXI_HP0_ARCACHE   (4'b0011),
        .S_AXI_HP0_ARPROT    (3'b000),
        .S_AXI_HP0_ARQOS     (4'b0000),
        .S_AXI_HP0_ARVALID   (s_axi_hp0_arvalid),
        .S_AXI_HP0_ARREADY   (s_axi_hp0_arready),
        .S_AXI_HP0_RID       (s_axi_hp0_rid),
        .S_AXI_HP0_RDATA     (s_axi_hp0_rdata),
        .S_AXI_HP0_RRESP     (s_axi_hp0_rresp),
        .S_AXI_HP0_RLAST     (s_axi_hp0_rlast),
        .S_AXI_HP0_RVALID    (s_axi_hp0_rvalid),
        .S_AXI_HP0_RREADY    (s_axi_hp0_rready),
        // HP0 write channel — tied off (DMA is read-only for MM2S)
        .S_AXI_HP0_AWID      (6'd0),
        .S_AXI_HP0_AWADDR    (32'd0),
        .S_AXI_HP0_AWLEN     (4'd0),
        .S_AXI_HP0_AWSIZE    (3'd0),
        .S_AXI_HP0_AWBURST   (2'd0),
        .S_AXI_HP0_AWLOCK    (2'b00),
        .S_AXI_HP0_AWCACHE   (4'b0000),
        .S_AXI_HP0_AWPROT    (3'b000),
        .S_AXI_HP0_AWQOS     (4'b0000),
        .S_AXI_HP0_AWVALID   (1'b0),
        .S_AXI_HP0_AWREADY   (),
        .S_AXI_HP0_WID       (6'd0),
        .S_AXI_HP0_WDATA     (64'd0),
        .S_AXI_HP0_WSTRB     (8'd0),
        .S_AXI_HP0_WLAST     (1'b0),
        .S_AXI_HP0_WVALID    (1'b0),
        .S_AXI_HP0_WREADY    (),
        .S_AXI_HP0_BID       (),
        .S_AXI_HP0_BRESP     (),
        .S_AXI_HP0_BVALID    (),
        .S_AXI_HP0_BREADY    (1'b1),

        // Interrupts (fabric to PS — 1-bit, all sources OR'd)
        .IRQ_F2P            (irq_f2p_combined)
    );

    // PS7 configured with 1 interrupt input. OR all sources.
    // Firmware polls status registers to determine source.
    wire irq_f2p_combined;
    assign irq_f2p_combined = irq_done | irq_error | irq_freq | irq_dma;


    //=========================================================================
    // AXI Interconnect (routes PS master to FBC peripherals)
    //=========================================================================
    // Address map (matches firmware/src/regs.rs):
    //   0x4004_0000 - 0x4004_3FFF: FBC Control (axi_fbc)
    //   0x4005_0000 - 0x4005_0FFF: I/O Config  (axi_io)
    //   0x4006_0000 - 0x4006_0FFF: Vector Status (axi_status)
    //   0x4007_0000 - 0x4007_0FFF: Freq Counter (axi_freq)
    //   0x4008_0000 - 0x4008_0FFF: Clock Ctrl  (axi_clk) - ONETWO freq select
    //
    // Decode: addr[19:16] selects peripheral group
    //   0x4004_xxxx = FBC Control     0x4005_xxxx = I/O Config
    //   0x4006_xxxx = Vector Status   0x4007_xxxx = Freq Counter
    //   0x4008_xxxx = Clock Ctrl      0x4009_xxxx = Error BRAMs
    //   0x4040_xxxx = DMA Controller
    wire fbc_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h4);
    wire io_sel     = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h5);
    wire status_sel = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h6);
    wire freq_sel_w = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h7);
    wire clk_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h8);
    wire err_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h9);
    wire dma_sel    = (m_axi_gp0_awaddr[31:20] == 12'h404);

    // Route write address channel
    assign axi_fbc_awaddr     = m_axi_gp0_awaddr[13:0];
    assign axi_fbc_awvalid    = m_axi_gp0_awvalid && fbc_sel;
    assign axi_io_awaddr      = m_axi_gp0_awaddr[11:0];
    assign axi_io_awvalid     = m_axi_gp0_awvalid && io_sel;
    assign axi_status_awaddr  = m_axi_gp0_awaddr[11:0];
    assign axi_status_awvalid = m_axi_gp0_awvalid && status_sel;
    assign axi_freq_awaddr    = m_axi_gp0_awaddr[11:0];
    assign axi_freq_awvalid   = m_axi_gp0_awvalid && freq_sel_w;
    assign axi_clk_awaddr     = m_axi_gp0_awaddr[11:0];
    assign axi_clk_awvalid    = m_axi_gp0_awvalid && clk_sel;
    assign axi_err_awaddr     = m_axi_gp0_awaddr[11:0];
    assign axi_err_awvalid    = m_axi_gp0_awvalid && err_sel;
    assign axi_dma_awaddr     = m_axi_gp0_awaddr[11:0];
    assign axi_dma_awvalid    = m_axi_gp0_awvalid && dma_sel;
    assign m_axi_gp0_awready = fbc_sel    ? axi_fbc_awready    :
                               io_sel     ? axi_io_awready     :
                               status_sel ? axi_status_awready :
                               freq_sel_w ? axi_freq_awready   :
                               clk_sel    ? axi_clk_awready    :
                               err_sel    ? axi_err_awready    :
                               dma_sel    ? axi_dma_awready    : 1'b1;

    // Route write data channel
    assign axi_fbc_wdata      = m_axi_gp0_wdata;
    assign axi_fbc_wstrb      = m_axi_gp0_wstrb;
    assign axi_fbc_wvalid     = m_axi_gp0_wvalid && fbc_sel;
    assign axi_io_wdata       = m_axi_gp0_wdata;
    assign axi_io_wstrb       = m_axi_gp0_wstrb;
    assign axi_io_wvalid      = m_axi_gp0_wvalid && io_sel;
    assign axi_status_wdata   = m_axi_gp0_wdata;
    assign axi_status_wstrb   = m_axi_gp0_wstrb;
    assign axi_status_wvalid  = m_axi_gp0_wvalid && status_sel;
    assign axi_freq_wdata     = m_axi_gp0_wdata;
    assign axi_freq_wstrb     = m_axi_gp0_wstrb;
    assign axi_freq_wvalid    = m_axi_gp0_wvalid && freq_sel_w;
    assign axi_clk_wdata      = m_axi_gp0_wdata;
    assign axi_clk_wstrb      = m_axi_gp0_wstrb;
    assign axi_clk_wvalid     = m_axi_gp0_wvalid && clk_sel;
    assign axi_err_wdata      = m_axi_gp0_wdata;
    assign axi_err_wstrb      = m_axi_gp0_wstrb;
    assign axi_err_wvalid     = m_axi_gp0_wvalid && err_sel;
    assign axi_dma_wdata      = m_axi_gp0_wdata;
    assign axi_dma_wstrb      = m_axi_gp0_wstrb;
    assign axi_dma_wvalid     = m_axi_gp0_wvalid && dma_sel;
    assign m_axi_gp0_wready = fbc_sel    ? axi_fbc_wready    :
                              io_sel     ? axi_io_wready     :
                              status_sel ? axi_status_wready :
                              freq_sel_w ? axi_freq_wready   :
                              clk_sel    ? axi_clk_wready    :
                              err_sel    ? axi_err_wready    :
                              dma_sel    ? axi_dma_wready    : 1'b1;

    // Route write response channel
    assign m_axi_gp0_bresp  = fbc_sel    ? axi_fbc_bresp    :
                              io_sel     ? axi_io_bresp     :
                              status_sel ? axi_status_bresp :
                              freq_sel_w ? axi_freq_bresp   :
                              clk_sel    ? axi_clk_bresp    :
                              err_sel    ? axi_err_bresp    :
                              dma_sel    ? axi_dma_bresp    : 2'b00;
    assign m_axi_gp0_bvalid = fbc_sel    ? axi_fbc_bvalid    :
                              io_sel     ? axi_io_bvalid     :
                              status_sel ? axi_status_bvalid :
                              freq_sel_w ? axi_freq_bvalid   :
                              clk_sel    ? axi_clk_bvalid    :
                              err_sel    ? axi_err_bvalid    :
                              dma_sel    ? axi_dma_bvalid    : 1'b0;
    assign axi_fbc_bready    = m_axi_gp0_bready && fbc_sel;
    assign axi_io_bready     = m_axi_gp0_bready && io_sel;
    assign axi_status_bready = m_axi_gp0_bready && status_sel;
    assign axi_freq_bready   = m_axi_gp0_bready && freq_sel_w;
    assign axi_clk_bready    = m_axi_gp0_bready && clk_sel;
    assign axi_err_bready    = m_axi_gp0_bready && err_sel;
    assign axi_dma_bready    = m_axi_gp0_bready && dma_sel;

    // Route read address channel (use same decode on araddr)
    wire fbc_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h4);
    wire io_sel_rd     = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h5);
    wire status_sel_rd = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h6);
    wire freq_sel_rd   = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h7);
    wire clk_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h8);
    wire err_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h9);
    wire dma_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h404);

    assign axi_fbc_araddr     = m_axi_gp0_araddr[13:0];
    assign axi_fbc_arvalid    = m_axi_gp0_arvalid && fbc_sel_rd;
    assign axi_io_araddr      = m_axi_gp0_araddr[11:0];
    assign axi_io_arvalid     = m_axi_gp0_arvalid && io_sel_rd;
    assign axi_status_araddr  = m_axi_gp0_araddr[11:0];
    assign axi_status_arvalid = m_axi_gp0_arvalid && status_sel_rd;
    assign axi_freq_araddr    = m_axi_gp0_araddr[11:0];
    assign axi_freq_arvalid   = m_axi_gp0_arvalid && freq_sel_rd;
    assign axi_clk_araddr     = m_axi_gp0_araddr[11:0];
    assign axi_clk_arvalid    = m_axi_gp0_arvalid && clk_sel_rd;
    assign axi_err_araddr     = m_axi_gp0_araddr[11:0];
    assign axi_err_arvalid    = m_axi_gp0_arvalid && err_sel_rd;
    assign axi_dma_araddr     = m_axi_gp0_araddr[11:0];
    assign axi_dma_arvalid    = m_axi_gp0_arvalid && dma_sel_rd;
    assign m_axi_gp0_arready = fbc_sel_rd    ? axi_fbc_arready    :
                               io_sel_rd     ? axi_io_arready     :
                               status_sel_rd ? axi_status_arready :
                               freq_sel_rd   ? axi_freq_arready   :
                               clk_sel_rd    ? axi_clk_arready    :
                               err_sel_rd    ? axi_err_arready    :
                               dma_sel_rd    ? axi_dma_arready    : 1'b1;

    // Route read data channel
    assign m_axi_gp0_rdata  = fbc_sel_rd    ? axi_fbc_rdata    :
                              io_sel_rd     ? axi_io_rdata     :
                              status_sel_rd ? axi_status_rdata :
                              freq_sel_rd   ? axi_freq_rdata   :
                              clk_sel_rd    ? axi_clk_rdata    :
                              err_sel_rd    ? axi_err_rdata    :
                              dma_sel_rd    ? axi_dma_rdata    : 32'h0;
    assign m_axi_gp0_rresp  = fbc_sel_rd    ? axi_fbc_rresp    :
                              io_sel_rd     ? axi_io_rresp     :
                              status_sel_rd ? axi_status_rresp :
                              freq_sel_rd   ? axi_freq_rresp   :
                              clk_sel_rd    ? axi_clk_rresp    :
                              err_sel_rd    ? axi_err_rresp    :
                              dma_sel_rd    ? axi_dma_rresp    : 2'b00;
    assign m_axi_gp0_rvalid = fbc_sel_rd    ? axi_fbc_rvalid    :
                              io_sel_rd     ? axi_io_rvalid     :
                              status_sel_rd ? axi_status_rvalid :
                              freq_sel_rd   ? axi_freq_rvalid   :
                              clk_sel_rd    ? axi_clk_rvalid    :
                              err_sel_rd    ? axi_err_rvalid    :
                              dma_sel_rd    ? axi_dma_rvalid    : 1'b0;
    assign axi_fbc_rready    = m_axi_gp0_rready && fbc_sel_rd;
    assign axi_io_rready     = m_axi_gp0_rready && io_sel_rd;
    assign axi_status_rready = m_axi_gp0_rready && status_sel_rd;
    assign axi_freq_rready   = m_axi_gp0_rready && freq_sel_rd;
    assign axi_clk_rready    = m_axi_gp0_rready && clk_sel_rd;
    assign axi_err_rready    = m_axi_gp0_rready && err_sel_rd;
    assign axi_dma_rready    = m_axi_gp0_rready && dma_sel_rd;

    // AXI ID passthrough (single master, IDs don't matter much)
    assign m_axi_gp0_bid = m_axi_gp0_awid;
    assign m_axi_gp0_rid = m_axi_gp0_arid;
    assign m_axi_gp0_rlast = 1'b1;  // Single-beat transactions

    //=========================================================================
    // FBC DMA Controller (0x4040_0000)
    //=========================================================================
    // Custom DMA: reads from DDR/OCM via S_AXI_HP0, outputs 256-bit
    // AXI-Stream to FBC decoder. Register interface matches Xilinx DMA
    // layout so firmware dma.rs works without changes.
    fbc_dma #(
        .AXI_ADDR_WIDTH(12),
        .AXI_DATA_WIDTH(32)
    ) u_fbc_dma (
        .clk            (clk_100m),
        .resetn         (sys_resetn),

        // AXI-Lite slave (register access from PS)
        .s_axi_awaddr   (axi_dma_awaddr),
        .s_axi_awvalid  (axi_dma_awvalid),
        .s_axi_awready  (axi_dma_awready),
        .s_axi_wdata    (axi_dma_wdata),
        .s_axi_wstrb    (axi_dma_wstrb),
        .s_axi_wvalid   (axi_dma_wvalid),
        .s_axi_wready   (axi_dma_wready),
        .s_axi_bresp    (axi_dma_bresp),
        .s_axi_bvalid   (axi_dma_bvalid),
        .s_axi_bready   (axi_dma_bready),
        .s_axi_araddr   (axi_dma_araddr),
        .s_axi_arvalid  (axi_dma_arvalid),
        .s_axi_arready  (axi_dma_arready),
        .s_axi_rdata    (axi_dma_rdata),
        .s_axi_rresp    (axi_dma_rresp),
        .s_axi_rvalid   (axi_dma_rvalid),
        .s_axi_rready   (axi_dma_rready),

        // AXI master read (to PS7 S_AXI_HP0)
        .m_axi_araddr   (s_axi_hp0_araddr),
        .m_axi_arlen    (s_axi_hp0_arlen),
        .m_axi_arsize   (s_axi_hp0_arsize),
        .m_axi_arburst  (s_axi_hp0_arburst),
        .m_axi_arvalid  (s_axi_hp0_arvalid),
        .m_axi_arready  (s_axi_hp0_arready),
        .m_axi_arid     (s_axi_hp0_arid),
        .m_axi_rdata    (s_axi_hp0_rdata),
        .m_axi_rresp    (s_axi_hp0_rresp),
        .m_axi_rvalid   (s_axi_hp0_rvalid),
        .m_axi_rready   (s_axi_hp0_rready),
        .m_axi_rlast    (s_axi_hp0_rlast),
        .m_axi_rid      (s_axi_hp0_rid),

        // AXI-Stream master (256-bit to FBC decoder)
        .m_axis_tdata   (axis_tdata),
        .m_axis_tvalid  (axis_tvalid),
        .m_axis_tready  (axis_tready),
        .m_axis_tlast   (axis_tlast),
        .m_axis_tkeep   (axis_tkeep),

        // Interrupt
        .irq            (irq_dma)
    );

    //=========================================================================
    // Error BRAMs (3 instances: pattern, vector number, cycle count)
    //=========================================================================
    // Port A: write from error_counter (inside fbc_top)
    // Port B: read from firmware via AXI at 0x4009_0000

    // Error pattern BRAM (128-bit wide, 1024 deep)
    error_bram #(
        .DATA_WIDTH(`VECTOR_WIDTH),
        .ADDR_WIDTH(10),
        .DEPTH(`ERROR_BRAM_DEPTH)
    ) u_err_pat_bram (
        .clk_a   (vec_clk),
        .addr_a  (err_pat_addr[9:0]),
        .din_a   (err_pat_data),
        .we_a    (err_pat_we),
        .clk_b   (clk_100m),
        .addr_b  (err_rd_addr),
        .dout_b  (err_pat_rd)
    );

    // Vector number BRAM (32-bit wide, 1024 deep)
    error_bram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(10),
        .DEPTH(`ERROR_BRAM_DEPTH)
    ) u_err_vec_bram (
        .clk_a   (vec_clk),
        .addr_a  (err_vec_addr[9:0]),
        .din_a   (err_vec_data),
        .we_a    (err_vec_we),
        .clk_b   (clk_100m),
        .addr_b  (err_rd_addr),
        .dout_b  (err_vec_rd)
    );

    // Cycle count BRAM (64-bit wide, 1024 deep)
    error_bram #(
        .DATA_WIDTH(64),
        .ADDR_WIDTH(10),
        .DEPTH(`ERROR_BRAM_DEPTH)
    ) u_err_cyc_bram (
        .clk_a   (vec_clk),
        .addr_a  (err_cyc_addr[9:0]),
        .din_a   (err_cyc_data),
        .we_a    (err_cyc_we),
        .clk_b   (clk_100m),
        .addr_b  (err_rd_addr),
        .dout_b  (err_cyc_rd)
    );

    //=========================================================================
    // Error BRAM AXI-Lite Read Interface (0x4009_0000)
    //=========================================================================
    // Register map:
    //   0x00: Error index (write to select which error to read)
    //   0x04: Error pattern [31:0]
    //   0x08: Error pattern [63:32]
    //   0x0C: Error pattern [95:64]
    //   0x10: Error pattern [127:96]
    //   0x14: Vector number at error
    //   0x18: Cycle count [31:0]
    //   0x1C: Cycle count [63:32]

    // Write side: capture error index
    assign axi_err_awready = 1'b1;
    assign axi_err_wready  = 1'b1;
    assign axi_err_bresp   = 2'b00;
    assign axi_err_bvalid  = axi_err_wvalid && err_sel;

    always @(posedge clk_100m or negedge sys_resetn) begin
        if (!sys_resetn) begin
            err_rd_addr <= 10'd0;
        end else if (axi_err_wvalid && err_sel && axi_err_awaddr[7:0] == 8'h00) begin
            err_rd_addr <= axi_err_wdata[9:0];
        end
    end

    // Read side: mux BRAM outputs based on address offset
    reg        err_rd_valid;
    reg [31:0] err_rd_data;

    assign axi_err_arready = !err_rd_valid;
    assign axi_err_rdata   = err_rd_data;
    assign axi_err_rresp   = 2'b00;
    assign axi_err_rvalid  = err_rd_valid;

    always @(posedge clk_100m or negedge sys_resetn) begin
        if (!sys_resetn) begin
            err_rd_valid <= 1'b0;
            err_rd_data  <= 32'd0;
        end else begin
            if (err_rd_valid && axi_err_rready) begin
                err_rd_valid <= 1'b0;
            end
            if (axi_err_arvalid && err_sel_rd && !err_rd_valid) begin
                err_rd_valid <= 1'b1;
                case (axi_err_araddr[7:0])
                    8'h00: err_rd_data <= {22'd0, err_rd_addr};
                    8'h04: err_rd_data <= err_pat_rd[31:0];
                    8'h08: err_rd_data <= err_pat_rd[63:32];
                    8'h0C: err_rd_data <= err_pat_rd[95:64];
                    8'h10: err_rd_data <= err_pat_rd[127:96];
                    8'h14: err_rd_data <= err_vec_rd;
                    8'h18: err_rd_data <= err_cyc_rd[31:0];
                    8'h1C: err_rd_data <= err_cyc_rd[63:32];
                    default: err_rd_data <= 32'd0;
                endcase
            end
        end
    end

`else
    // Simulation: Stub signals driven by testbench
    reg sim_clk_100m = 0;
    reg sim_clk_200m = 0;
    reg sim_resetn = 1;

    assign clk_100m = sim_clk_100m;
    assign clk_200m = sim_clk_200m;
    assign ps_resetn = sim_resetn;

    // Stub AXI interfaces for simulation
    assign axi_fbc_awaddr  = 14'b0;
    assign axi_fbc_awvalid = 1'b0;
    assign axi_fbc_wdata   = 32'b0;
    assign axi_fbc_wstrb   = 4'b0;
    assign axi_fbc_wvalid  = 1'b0;
    assign axi_fbc_bready  = 1'b1;
    assign axi_fbc_araddr  = 14'b0;
    assign axi_fbc_arvalid = 1'b0;
    assign axi_fbc_rready  = 1'b1;

    assign axi_io_awaddr   = 12'b0;
    assign axi_io_awvalid  = 1'b0;
    assign axi_io_wdata    = 32'b0;
    assign axi_io_wstrb    = 4'b0;
    assign axi_io_wvalid   = 1'b0;
    assign axi_io_bready   = 1'b1;
    assign axi_io_araddr   = 12'b0;
    assign axi_io_arvalid  = 1'b0;
    assign axi_io_rready   = 1'b1;

    assign axi_status_awaddr  = 12'b0;
    assign axi_status_awvalid = 1'b0;
    assign axi_status_wdata   = 32'b0;
    assign axi_status_wstrb   = 4'b0;
    assign axi_status_wvalid  = 1'b0;
    assign axi_status_bready  = 1'b1;
    assign axi_status_araddr  = 12'b0;
    assign axi_status_arvalid = 1'b0;
    assign axi_status_rready  = 1'b1;

    assign axi_clk_awaddr  = 12'b0;
    assign axi_clk_awvalid = 1'b0;
    assign axi_clk_wdata   = 32'b0;
    assign axi_clk_wstrb   = 4'b0;
    assign axi_clk_wvalid  = 1'b0;
    assign axi_clk_bready  = 1'b1;
    assign axi_clk_araddr  = 12'b0;
    assign axi_clk_arvalid = 1'b0;
    assign axi_clk_rready  = 1'b1;

    assign axi_freq_awaddr  = 12'b0;
    assign axi_freq_awvalid = 1'b0;
    assign axi_freq_wdata   = 32'b0;
    assign axi_freq_wstrb   = 4'b0;
    assign axi_freq_wvalid  = 1'b0;
    assign axi_freq_bready  = 1'b1;
    assign axi_freq_araddr  = 12'b0;
    assign axi_freq_arvalid = 1'b0;
    assign axi_freq_rready  = 1'b1;

    assign axis_tdata  = 256'b0;
    assign axis_tvalid = 1'b0;
    assign axis_tlast  = 1'b0;
    assign axis_tkeep  = 32'b0;
`endif

endmodule
