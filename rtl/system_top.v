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
// Target: Zynq-7020 (xc7z020clg400-1)
//
//=============================================================================

`include "fbc_pkg.vh"

module system_top (
    //=========================================================================
    // DDR Pins (directly to Zynq)
    //=========================================================================
    inout wire [14:0] DDR_addr,
    inout wire [2:0]  DDR_ba,
    inout wire        DDR_cas_n,
    inout wire        DDR_ck_n,
    inout wire        DDR_ck_p,
    inout wire        DDR_cke,
    inout wire        DDR_cs_n,
    inout wire [3:0]  DDR_dm,
    inout wire [31:0] DDR_dq,
    inout wire [3:0]  DDR_dqs_n,
    inout wire [3:0]  DDR_dqs_p,
    inout wire        DDR_odt,
    inout wire        DDR_ras_n,
    inout wire        DDR_reset_n,
    inout wire        DDR_we_n,

    //=========================================================================
    // Fixed IO (PS peripherals)
    //=========================================================================
    inout wire        FIXED_IO_ddr_vrn,
    inout wire        FIXED_IO_ddr_vrp,
    inout wire [53:0] FIXED_IO_mio,
    inout wire        FIXED_IO_ps_clk,
    inout wire        FIXED_IO_ps_porb,
    inout wire        FIXED_IO_ps_srstb,

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
    output wire [3:0] clk_out_n,

    //=========================================================================
    // Status LEDs (directly connected)
    //=========================================================================
    output wire [3:0] led
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

        // Fast pin errors (optional - for debugging)
        .fast_error     (),               // TODO: connect to status register

        // Error BRAMs (directly connect to BRAM controller later)
        .error_bram_addr     (),
        .error_bram_data     (),
        .error_bram_we       (),
        .error_vec_bram_addr (),
        .error_vec_bram_data (),
        .error_vec_bram_we   (),
        .error_cyc_bram_addr (),
        .error_cyc_bram_data (),
        .error_cyc_bram_we   (),

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

    //=========================================================================
    // Status LEDs
    //=========================================================================
    assign led[0] = clk_locked;         // Clock locked
    assign led[1] = vec_clk_en;         // Running
    assign led[2] = irq_done;           // Test complete
    assign led[3] = irq_error;          // Error detected

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

    // AXI Stream DMA interface (HP0 simplified to stream)
    wire [63:0] s_axi_hp0_wdata;
    wire        s_axi_hp0_wvalid, s_axi_hp0_wready;
    wire        s_axi_hp0_wlast;

`ifndef SIMULATION
    // Synthesis: Instantiate actual PS7 block
    processing_system7_0 u_ps (
        // DDR Interface
        .DDR_addr           (DDR_addr),
        .DDR_ba             (DDR_ba),
        .DDR_cas_n          (DDR_cas_n),
        .DDR_ck_n           (DDR_ck_n),
        .DDR_ck_p           (DDR_ck_p),
        .DDR_cke            (DDR_cke),
        .DDR_cs_n           (DDR_cs_n),
        .DDR_dm             (DDR_dm),
        .DDR_dq             (DDR_dq),
        .DDR_dqs_n          (DDR_dqs_n),
        .DDR_dqs_p          (DDR_dqs_p),
        .DDR_odt            (DDR_odt),
        .DDR_ras_n          (DDR_ras_n),
        .DDR_reset_n        (DDR_reset_n),
        .DDR_we_n           (DDR_we_n),

        // Fixed IO
        .FIXED_IO_ddr_vrn   (FIXED_IO_ddr_vrn),
        .FIXED_IO_ddr_vrp   (FIXED_IO_ddr_vrp),
        .FIXED_IO_mio       (FIXED_IO_mio),
        .FIXED_IO_ps_clk    (FIXED_IO_ps_clk),
        .FIXED_IO_ps_porb   (FIXED_IO_ps_porb),
        .FIXED_IO_ps_srstb  (FIXED_IO_ps_srstb),

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

        // Interrupts (fabric to PS)
        .IRQ_F2P            (irq_f2p_combined)
    );

    wire [15:0] irq_f2p_combined;
    assign irq_f2p_combined = {13'b0, irq_freq, irq_error, irq_done};


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
    wire fbc_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h4);
    wire io_sel     = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h5);
    wire status_sel = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h6);
    wire freq_sel_w = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h7);
    wire clk_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h8);

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
    assign m_axi_gp0_awready = fbc_sel    ? axi_fbc_awready    :
                               io_sel     ? axi_io_awready     :
                               status_sel ? axi_status_awready :
                               freq_sel_w ? axi_freq_awready   :
                               clk_sel    ? axi_clk_awready    : 1'b1;

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
    assign m_axi_gp0_wready = fbc_sel    ? axi_fbc_wready    :
                              io_sel     ? axi_io_wready     :
                              status_sel ? axi_status_wready :
                              freq_sel_w ? axi_freq_wready   :
                              clk_sel    ? axi_clk_wready    : 1'b1;

    // Route write response channel
    assign m_axi_gp0_bresp  = fbc_sel    ? axi_fbc_bresp    :
                              io_sel     ? axi_io_bresp     :
                              status_sel ? axi_status_bresp :
                              freq_sel_w ? axi_freq_bresp   :
                              clk_sel    ? axi_clk_bresp    : 2'b00;
    assign m_axi_gp0_bvalid = fbc_sel    ? axi_fbc_bvalid    :
                              io_sel     ? axi_io_bvalid     :
                              status_sel ? axi_status_bvalid :
                              freq_sel_w ? axi_freq_bvalid   :
                              clk_sel    ? axi_clk_bvalid    : 1'b0;
    assign axi_fbc_bready    = m_axi_gp0_bready && fbc_sel;
    assign axi_io_bready     = m_axi_gp0_bready && io_sel;
    assign axi_status_bready = m_axi_gp0_bready && status_sel;
    assign axi_freq_bready   = m_axi_gp0_bready && freq_sel_w;
    assign axi_clk_bready    = m_axi_gp0_bready && clk_sel;

    // Route read address channel (use same decode on araddr)
    wire fbc_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h4);
    wire io_sel_rd     = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h5);
    wire status_sel_rd = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h6);
    wire freq_sel_rd   = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h7);
    wire clk_sel_rd    = (m_axi_gp0_araddr[31:20] == 12'h400) && (m_axi_gp0_araddr[19:16] == 4'h8);

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
    assign m_axi_gp0_arready = fbc_sel_rd    ? axi_fbc_arready    :
                               io_sel_rd     ? axi_io_arready     :
                               status_sel_rd ? axi_status_arready :
                               freq_sel_rd   ? axi_freq_arready   :
                               clk_sel_rd    ? axi_clk_arready    : 1'b1;

    // Route read data channel
    assign m_axi_gp0_rdata  = fbc_sel_rd    ? axi_fbc_rdata    :
                              io_sel_rd     ? axi_io_rdata     :
                              status_sel_rd ? axi_status_rdata :
                              freq_sel_rd   ? axi_freq_rdata   :
                              clk_sel_rd    ? axi_clk_rdata    : 32'h0;
    assign m_axi_gp0_rresp  = fbc_sel_rd    ? axi_fbc_rresp    :
                              io_sel_rd     ? axi_io_rresp     :
                              status_sel_rd ? axi_status_rresp :
                              freq_sel_rd   ? axi_freq_rresp   :
                              clk_sel_rd    ? axi_clk_rresp    : 2'b00;
    assign m_axi_gp0_rvalid = fbc_sel_rd    ? axi_fbc_rvalid    :
                              io_sel_rd     ? axi_io_rvalid     :
                              status_sel_rd ? axi_status_rvalid :
                              freq_sel_rd   ? axi_freq_rvalid   :
                              clk_sel_rd    ? axi_clk_rvalid    : 1'b0;
    assign axi_fbc_rready    = m_axi_gp0_rready && fbc_sel_rd;
    assign axi_io_rready     = m_axi_gp0_rready && io_sel_rd;
    assign axi_status_rready = m_axi_gp0_rready && status_sel_rd;
    assign axi_freq_rready   = m_axi_gp0_rready && freq_sel_rd;
    assign axi_clk_rready    = m_axi_gp0_rready && clk_sel_rd;

    // AXI ID passthrough (single master, IDs don't matter much)
    assign m_axi_gp0_bid = m_axi_gp0_awid;
    assign m_axi_gp0_rid = m_axi_gp0_arid;
    assign m_axi_gp0_rlast = 1'b1;  // Single-beat transactions

    //=========================================================================
    // AXI Stream from DMA (placeholder - connect to HP0 in full design)
    //=========================================================================
    // In full design, an AXI DMA IP reads from DDR via S_AXI_HP0 and
    // outputs AXI Stream to the FBC core. For now, stub the stream.
    assign axis_tdata  = {4{s_axi_hp0_wdata}};  // Expand 64b to 256b
    assign axis_tvalid = s_axi_hp0_wvalid;
    assign s_axi_hp0_wready = axis_tready;
    assign axis_tlast  = s_axi_hp0_wlast;
    assign axis_tkeep  = 32'hFFFFFFFF;

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
