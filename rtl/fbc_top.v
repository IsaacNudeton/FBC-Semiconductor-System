`timescale 1ns / 1ps
//=============================================================================
// FBC Semiconductor System - Top Level
//=============================================================================
//
// Integrates:
// - AXI Stream interface (DMA → FBC)
// - FBC decoder (bytecode → vectors)
// - Vector engine (vectors → pins)
// - I/O bank with 160 pins (128 BIM + 32 fast)
// - Error counter (error logging)
// - AXI-Lite control registers
//
// Pin Architecture:
//   gpio[0:127]   - BIM pins (through Quad Board to DUT, 2-stage pipeline)
//   gpio[128:159] - Fast pins (direct FPGA, 1-stage for triggers/clocks)
//
//=============================================================================

`include "fbc_pkg.vh"

module fbc_top #(
    parameter VECTOR_WIDTH = `VECTOR_WIDTH,   // 128 BIM pins
    parameter FAST_WIDTH = `FAST_WIDTH,       // 32 fast pins
    parameter PIN_COUNT = `PIN_COUNT,         // 160 total
    parameter AXIS_DATA_WIDTH = 256
)(
    //=========================================================================
    // Clocks and Reset
    //=========================================================================
    input wire clk,              // Main AXI clock (100 MHz)
    input wire vec_clk,          // Vector execution clock
    input wire delay_clk,        // Fast clock for I/O timing (200+ MHz)
    input wire resetn,           // Active-low reset

    //=========================================================================
    // AXI4-Stream Slave (from DMA)
    //=========================================================================
    input wire [AXIS_DATA_WIDTH-1:0] s_axis_tdata,
    input wire                       s_axis_tvalid,
    output wire                      s_axis_tready,
    input wire                       s_axis_tlast,
    input wire [AXIS_DATA_WIDTH/8-1:0] s_axis_tkeep,

    //=========================================================================
    // AXI4-Lite Slave - FBC Control (directly memory mapped)
    //=========================================================================
    input wire [13:0]  s_axi_fbc_awaddr,
    input wire         s_axi_fbc_awvalid,
    output wire        s_axi_fbc_awready,
    input wire [31:0]  s_axi_fbc_wdata,
    input wire [3:0]   s_axi_fbc_wstrb,
    input wire         s_axi_fbc_wvalid,
    output wire        s_axi_fbc_wready,
    output wire [1:0]  s_axi_fbc_bresp,
    output wire        s_axi_fbc_bvalid,
    input wire         s_axi_fbc_bready,
    input wire [13:0]  s_axi_fbc_araddr,
    input wire         s_axi_fbc_arvalid,
    output wire        s_axi_fbc_arready,
    output wire [31:0] s_axi_fbc_rdata,
    output wire [1:0]  s_axi_fbc_rresp,
    output wire        s_axi_fbc_rvalid,
    input wire         s_axi_fbc_rready,

    //=========================================================================
    // Pin Interface - All 160 Pins
    //=========================================================================
    output wire [PIN_COUNT-1:0] pin_dout,      // Output data (160 pins)
    output wire [PIN_COUNT-1:0] pin_oen,       // Output enable (160 pins)
    input wire [PIN_COUNT-1:0]  pin_din,       // Input data (160 pins)

    //=========================================================================
    // Fast Pin Control (direct vector data for pins 128-159)
    // Note: fast_dout/fast_oen now controlled via AXI registers in axi_fbc_ctrl
    //=========================================================================
    input wire                   fast_clk_en,  // Fast pins clock enable

    //=========================================================================
    // AXI4-Lite Slave - I/O Configuration
    //=========================================================================
    input wire [11:0]  s_axi_io_awaddr,
    input wire         s_axi_io_awvalid,
    output wire        s_axi_io_awready,
    input wire [31:0]  s_axi_io_wdata,
    input wire [3:0]   s_axi_io_wstrb,
    input wire         s_axi_io_wvalid,
    output wire        s_axi_io_wready,
    output wire [1:0]  s_axi_io_bresp,
    output wire        s_axi_io_bvalid,
    input wire         s_axi_io_bready,
    input wire [11:0]  s_axi_io_araddr,
    input wire         s_axi_io_arvalid,
    output wire        s_axi_io_arready,
    output wire [31:0] s_axi_io_rdata,
    output wire [1:0]  s_axi_io_rresp,
    output wire        s_axi_io_rvalid,
    input wire         s_axi_io_rready,

    //=========================================================================
    // AXI4-Lite Slave - Vector Status (0x4006_0000)
    //=========================================================================
    input wire [11:0]  s_axi_status_awaddr,
    input wire         s_axi_status_awvalid,
    output wire        s_axi_status_awready,
    input wire [31:0]  s_axi_status_wdata,
    input wire [3:0]   s_axi_status_wstrb,
    input wire         s_axi_status_wvalid,
    output wire        s_axi_status_wready,
    output wire [1:0]  s_axi_status_bresp,
    output wire        s_axi_status_bvalid,
    input wire         s_axi_status_bready,
    input wire [11:0]  s_axi_status_araddr,
    input wire         s_axi_status_arvalid,
    output wire        s_axi_status_arready,
    output wire [31:0] s_axi_status_rdata,
    output wire [1:0]  s_axi_status_rresp,
    output wire        s_axi_status_rvalid,
    input wire         s_axi_status_rready,

    //=========================================================================
    // Error BRAM Interfaces (directly connect to BRAM controller)
    //=========================================================================
    output wire [31:0]             error_bram_addr,
    output wire [VECTOR_WIDTH-1:0] error_bram_data,
    output wire                    error_bram_we,

    output wire [31:0]             error_vec_bram_addr,
    output wire [31:0]             error_vec_bram_data,
    output wire                    error_vec_bram_we,

    output wire [31:0]             error_cyc_bram_addr,
    output wire [63:0]             error_cyc_bram_data,
    output wire                    error_cyc_bram_we,

    //=========================================================================
    // Error Outputs
    //=========================================================================
    output wire [FAST_WIDTH-1:0] fast_error,   // Fast pin error flags (32 pins)

    //=========================================================================
    // Interrupts
    //=========================================================================
    output wire irq_done,
    output wire irq_error
);

    //=========================================================================
    // Internal Signals
    //=========================================================================

    // AXI Stream → FBC
    wire [63:0]  fbc_instr;
    wire [127:0] fbc_payload;
    wire         fbc_valid;
    wire         fbc_ready;
    wire         fbc_last;

    // FBC Decoder → Vector Engine
    wire [VECTOR_WIDTH-1:0] dec_dout;
    wire [VECTOR_WIDTH-1:0] dec_oen;
    wire [31:0]             dec_repeat;
    wire                    dec_valid;
    wire                    dec_ready;

    // FBC Decoder status
    wire        dec_running;
    wire        dec_done;
    wire        dec_error;
    wire [31:0] dec_instr_count;
    wire [63:0] dec_cycle_count;

    // Vector Engine → Error Counter
    wire [VECTOR_WIDTH-1:0] error_mask;
    wire                    error_valid;
    wire [31:0]             vector_count;
    wire [63:0]             cycle_count;

    // Vector Engine status
    wire vec_running;
    wire vec_done;

    // FBC Control
    wire fbc_enable;
    wire fbc_reset;

    // Error Counter status
    wire [31:0] total_error_count;
    wire [31:0] first_error_vector;
    wire [63:0] first_error_cycle;
    wire        first_error_detected;
    wire        error_overflow;

    // I/O Configuration → I/O Bank (all 160 pins)
    wire [4*PIN_COUNT-1:0]  pin_type;
    wire [16*PIN_COUNT-1:0] pulse_ctrl_bits;

    // Vector Engine → I/O Bank (128 BIM pins)
    wire [VECTOR_WIDTH-1:0] vec_dout_internal;
    wire [VECTOR_WIDTH-1:0] vec_oen_internal;

    // I/O Bank → Error outputs
    wire [VECTOR_WIDTH-1:0] io_error;       // BIM pin errors (128)
    wire [FAST_WIDTH-1:0]   io_fast_error;  // Fast pin errors (32)

    // Fast pin control (from AXI registers in axi_fbc_ctrl)
    wire [FAST_WIDTH-1:0]   ctrl_fast_dout;  // Fast pin drive values
    wire [FAST_WIDTH-1:0]   ctrl_fast_oen;   // Fast pin output enables
    wire [FAST_WIDTH-1:0]   fast_din;        // Fast pin input states (for readback)

    //=========================================================================
    // I/O Configuration (AXI-Lite accessible pin settings for all 160 pins)
    //=========================================================================
    io_config #(
        .WIDTH(PIN_COUNT),
        .BIM_WIDTH(VECTOR_WIDTH),
        .FAST_WIDTH(FAST_WIDTH),
        .AXI_ADDR_WIDTH(12),
        .AXI_DATA_WIDTH(32)
    ) u_io_config (
        .clk(clk),
        .resetn(resetn),

        // AXI-Lite
        .s_axi_awaddr(s_axi_io_awaddr),
        .s_axi_awvalid(s_axi_io_awvalid),
        .s_axi_awready(s_axi_io_awready),
        .s_axi_wdata(s_axi_io_wdata),
        .s_axi_wstrb(s_axi_io_wstrb),
        .s_axi_wvalid(s_axi_io_wvalid),
        .s_axi_wready(s_axi_io_wready),
        .s_axi_bresp(s_axi_io_bresp),
        .s_axi_bvalid(s_axi_io_bvalid),
        .s_axi_bready(s_axi_io_bready),
        .s_axi_araddr(s_axi_io_araddr),
        .s_axi_arvalid(s_axi_io_arvalid),
        .s_axi_arready(s_axi_io_arready),
        .s_axi_rdata(s_axi_io_rdata),
        .s_axi_rresp(s_axi_io_rresp),
        .s_axi_rvalid(s_axi_io_rvalid),
        .s_axi_rready(s_axi_io_rready),

        // Configuration output
        .pin_type(pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits)
    );

    //=========================================================================
    // I/O Bank (160 I/O cells: 128 BIM + 32 fast)
    //=========================================================================
    io_bank #(
        .BIM_WIDTH(VECTOR_WIDTH),
        .FAST_WIDTH(FAST_WIDTH),
        .WIDTH(PIN_COUNT)
    ) u_io_bank (
        .delay_clk(delay_clk),
        .vec_clk(vec_clk),
        .resetn(resetn),

        // Timing
        .vec_clk_en(fbc_enable),

        // Configuration (all 160 pins)
        .pin_type(pin_type),
        .pulse_ctrl_bits(pulse_ctrl_bits),

        // BIM vector data (from vector engine, pins 0-127)
        .dout(vec_dout_internal),
        .oen(vec_oen_internal),

        // Fast vector data (from AXI registers, pins 128-159)
        .fast_dout(ctrl_fast_dout),
        .fast_oen(ctrl_fast_oen),
        .fast_clk_en(fast_clk_en),

        // Physical pins (all 160)
        .pin_din(pin_din),
        .pin_dout(pin_dout),
        .pin_oen(pin_oen),

        // Error outputs (split by type)
        .error(io_error),
        .fast_error(io_fast_error)
    );

    // Export fast errors
    assign fast_error = io_fast_error;
    // Fast pin input states for AXI readback
    assign fast_din = pin_din[PIN_COUNT-1:VECTOR_WIDTH];

    //=========================================================================
    // AXI Stream to FBC Interface
    //=========================================================================
    axi_stream_fbc #(
        .AXIS_DATA_WIDTH(AXIS_DATA_WIDTH)
    ) u_axi_stream_fbc (
        .clk(clk),
        .resetn(resetn),

        // AXI Stream
        .s_axis_tdata(s_axis_tdata),
        .s_axis_tvalid(s_axis_tvalid),
        .s_axis_tready(s_axis_tready),
        .s_axis_tlast(s_axis_tlast),
        .s_axis_tkeep(s_axis_tkeep),

        // FBC output
        .fbc_instr(fbc_instr),
        .fbc_payload(fbc_payload),
        .fbc_valid(fbc_valid),
        .fbc_ready(fbc_ready),
        .fbc_last(fbc_last),

        // Status
        .instr_received(),
        .stream_done()
    );

    //=========================================================================
    // FBC Decoder
    //=========================================================================
    fbc_decoder #(
        .VECTOR_WIDTH(VECTOR_WIDTH),
        .REPEAT_WIDTH(32)
    ) u_fbc_decoder (
        .clk(vec_clk),
        .resetn(resetn & ~fbc_reset),

        // FBC input
        .fbc_instr(fbc_instr),
        .fbc_payload(fbc_payload),
        .fbc_valid(fbc_valid & fbc_enable),
        .fbc_ready(fbc_ready),

        // Vector output
        .vec_dout(dec_dout),
        .vec_oen(dec_oen),
        .vec_repeat(dec_repeat),
        .vec_valid(dec_valid),
        .vec_ready(dec_ready),

        // Status
        .running(dec_running),
        .done(dec_done),
        .error(dec_error),
        .instr_count(dec_instr_count),
        .cycle_count(dec_cycle_count)
    );

    //=========================================================================
    // Vector Engine (repeat counter + vector data routing)
    //=========================================================================
    // Note: Pin type handling moved to io_bank for proper pulse timing
    vector_engine #(
        .VECTOR_WIDTH(VECTOR_WIDTH),
        .REPEAT_WIDTH(32)
    ) u_vector_engine (
        .clk(clk),
        .vec_clk(vec_clk),
        .resetn(resetn),

        // Vector input
        .in_dout(dec_dout),
        .in_oen(dec_oen),
        .in_repeat(dec_repeat),
        .in_valid(dec_valid),
        .in_ready(dec_ready),

        // Pin interface (routed through io_bank)
        .pin_dout(vec_dout_internal),
        .pin_oen(vec_oen_internal),
        .pin_din(pin_din[VECTOR_WIDTH-1:0]),  // BIM pins only

        // Pin configuration (BIM pins only, passed to io_bank for reference)
        .pin_type(pin_type[4*VECTOR_WIDTH-1:0]),

        // Error output (from io_bank)
        .error_mask(io_error),
        .error_valid(error_valid),
        .vector_count(vector_count),
        .cycle_count(cycle_count),

        // Status
        .running(vec_running),
        .done(vec_done),
        .enable(fbc_enable)
    );

    // Error mask comes from io_bank
    assign error_mask = io_error;

    //=========================================================================
    // Error Counter
    //=========================================================================
    error_counter #(
        .VECTOR_WIDTH(VECTOR_WIDTH),
        .MAX_ERRORS(`MAX_ERROR_COUNT),
        .ERROR_COUNT_WIDTH(32)
    ) u_error_counter (
        .clk(vec_clk),
        .resetn(resetn & ~fbc_reset),

        // Error input
        .error_mask(error_mask),
        .error_valid(error_valid),
        .vector_count(vector_count),
        .cycle_count(cycle_count),

        // BRAM interfaces
        .bram_addr(error_bram_addr),
        .bram_data(error_bram_data),
        .bram_we(error_bram_we),

        .vec_bram_addr(error_vec_bram_addr),
        .vec_bram_data(error_vec_bram_data),
        .vec_bram_we(error_vec_bram_we),

        .cyc_bram_addr(error_cyc_bram_addr),
        .cyc_bram_data(error_cyc_bram_data),
        .cyc_bram_we(error_cyc_bram_we),

        // Status
        .total_error_count(total_error_count),
        .first_error_vector(first_error_vector),
        .first_error_cycle(first_error_cycle),
        .first_error_detected(first_error_detected),
        .error_overflow(error_overflow)
    );

    //=========================================================================
    // AXI FBC Control
    //=========================================================================
    axi_fbc_ctrl #(
        .AXI_ADDR_WIDTH(14),
        .AXI_DATA_WIDTH(32)
    ) u_axi_fbc_ctrl (
        .clk(clk),
        .resetn(resetn),

        // AXI-Lite
        .awaddr(s_axi_fbc_awaddr),
        .awvalid(s_axi_fbc_awvalid),
        .awready(s_axi_fbc_awready),
        .wdata(s_axi_fbc_wdata),
        .wstrb(s_axi_fbc_wstrb),
        .wvalid(s_axi_fbc_wvalid),
        .wready(s_axi_fbc_wready),
        .bresp(s_axi_fbc_bresp),
        .bvalid(s_axi_fbc_bvalid),
        .bready(s_axi_fbc_bready),
        .araddr(s_axi_fbc_araddr),
        .arvalid(s_axi_fbc_arvalid),
        .arready(s_axi_fbc_arready),
        .rdata(s_axi_fbc_rdata),
        .rresp(s_axi_fbc_rresp),
        .rvalid(s_axi_fbc_rvalid),
        .rready(s_axi_fbc_rready),

        // FBC Decoder interface
        .fbc_enable(fbc_enable),
        .fbc_reset(fbc_reset),
        .fbc_running(dec_running),
        .fbc_done(dec_done),
        .fbc_error(dec_error),
        .fbc_instr_count(dec_instr_count),
        .fbc_cycle_count(dec_cycle_count),

        // Interrupts
        .irq_done(irq_done),
        .irq_error(irq_error),

        // Fast pin control (Bank 35)
        .fast_dout(ctrl_fast_dout),
        .fast_oen(ctrl_fast_oen),
        .fast_din(fast_din)
    );

    //=========================================================================
    // AXI Vector Status (0x4006_0000)
    //=========================================================================
    axi_vector_status #(
        .AXI_ADDR_WIDTH(12),
        .AXI_DATA_WIDTH(32)
    ) u_axi_vector_status (
        .clk(clk),
        .resetn(resetn),

        // AXI-Lite
        .s_axi_awaddr(s_axi_status_awaddr),
        .s_axi_awvalid(s_axi_status_awvalid),
        .s_axi_awready(s_axi_status_awready),
        .s_axi_wdata(s_axi_status_wdata),
        .s_axi_wstrb(s_axi_status_wstrb),
        .s_axi_wvalid(s_axi_status_wvalid),
        .s_axi_wready(s_axi_status_wready),
        .s_axi_bresp(s_axi_status_bresp),
        .s_axi_bvalid(s_axi_status_bvalid),
        .s_axi_bready(s_axi_status_bready),
        .s_axi_araddr(s_axi_status_araddr),
        .s_axi_arvalid(s_axi_status_arvalid),
        .s_axi_arready(s_axi_status_arready),
        .s_axi_rdata(s_axi_status_rdata),
        .s_axi_rresp(s_axi_status_rresp),
        .s_axi_rvalid(s_axi_status_rvalid),
        .s_axi_rready(s_axi_status_rready),

        // Status inputs (from error_counter and vector_engine)
        .error_count(total_error_count),
        .vector_count(vector_count),
        .cycle_count(cycle_count),
        .first_error_vector(first_error_vector),
        .first_error_cycle(first_error_cycle),
        .first_error_valid(first_error_detected),
        .done(vec_done),
        .has_errors(first_error_detected)
    );

    // Export fast errors
    assign fast_error = io_fast_error;
    // Fast pin input states for AXI readback
    assign fast_din = pin_din[159:128];

endmodule
