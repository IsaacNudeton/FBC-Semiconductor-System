`timescale 1ns / 1ps
//=============================================================================
// AXI4-Lite Frequency Counter Interface
//=============================================================================
//
// Measures signal frequency from DUT pins. Useful for:
// - Verifying DUT clock output
// - Measuring signal timing characteristics
// - Debugging I/O connectivity
//
// Based on reference/kzhang_v2_2016/axi_freq_counter.v, simplified for
// AXI4-Lite and adapted to project style.
//
// Register Map (active at 0x4007_0000):
//   Counter N base = N * 0x20 (32 bytes per counter, 4 counters total)
//
//   +0x00: CTRL       - Control [0]=enable, [1]=irq_en, [15:8]=sig_sel, [23:16]=trig_sel
//   +0x04: STATUS     - Status [0]=done, [1]=idle, [2]=waiting, [3]=running, [6]=timeout_err
//   +0x08: MAX_CYCLES - Max cycle count before done (R/W)
//   +0x0C: MAX_TIME   - Max time count before done (R/W)
//   +0x10: CYCLES     - Measured cycle count (RO)
//   +0x14: TIME       - Measured time count (RO)
//   +0x18: TIMEOUT    - Timeout threshold (R/W)
//   +0x1C: RESERVED
//
//   0x7C: VERSION     - Module version (RO)
//
//=============================================================================

`include "fbc_pkg.vh"

module axi_freq_counter #(
    parameter AXI_ADDR_WIDTH = 12,
    parameter AXI_DATA_WIDTH = 32,
    parameter NUM_COUNTERS   = 4
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // AXI4-Lite Slave Interface
    //=========================================================================
    input wire [AXI_ADDR_WIDTH-1:0] s_axi_awaddr,
    input wire                       s_axi_awvalid,
    output reg                       s_axi_awready,

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
    // Signal Inputs (from pin_din, directly from DUT)
    //=========================================================================
    input wire [`PIN_COUNT-1:0] all_inputs,

    //=========================================================================
    // Interrupt Output
    //=========================================================================
    output wire irq
);

    //=========================================================================
    // Counter State Machine States
    //=========================================================================
    localparam FREQ_IDLE = 4'd1;
    localparam FREQ_WAIT = 4'd2;
    localparam FREQ_RUN  = 4'd4;
    localparam FREQ_DONE = 4'd8;

    //=========================================================================
    // Register Offsets (within each counter's 32-byte block)
    //=========================================================================
    localparam REG_CTRL       = 3'd0;  // +0x00
    localparam REG_STATUS     = 3'd1;  // +0x04
    localparam REG_MAX_CYCLES = 3'd2;  // +0x08
    localparam REG_MAX_TIME   = 3'd3;  // +0x0C
    localparam REG_CYCLES     = 3'd4;  // +0x10
    localparam REG_TIME       = 3'd5;  // +0x14
    localparam REG_TIMEOUT    = 3'd6;  // +0x18

    //=========================================================================
    // Per-Counter Registers and State
    //=========================================================================
    reg [31:0] ctrl_reg      [NUM_COUNTERS-1:0];  // Control register
    reg [31:0] max_cycles    [NUM_COUNTERS-1:0];  // Max cycles to measure
    reg [31:0] max_time      [NUM_COUNTERS-1:0];  // Max time to measure
    reg [31:0] timeout_reg   [NUM_COUNTERS-1:0];  // Timeout threshold

    // Counter outputs (directly from freq_counter instances)
    wire [31:0] status_reg   [NUM_COUNTERS-1:0];
    wire [31:0] cycle_count  [NUM_COUNTERS-1:0];
    wire [31:0] time_count   [NUM_COUNTERS-1:0];
    wire        counter_irq  [NUM_COUNTERS-1:0];

    //=========================================================================
    // Initialize registers on reset
    //=========================================================================
    integer i;
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            for (i = 0; i < NUM_COUNTERS; i = i + 1) begin
                ctrl_reg[i]    <= 32'h01_ff_ff_00;  // disabled, sig=ff, trig=ff
                max_cycles[i]  <= 32'h1;
                max_time[i]    <= 32'hffffffff;
                timeout_reg[i] <= 32'hfffffff7;    // ~40 sec at 100MHz
            end
        end else if (wr_en) begin
            if (wr_reg == REG_CTRL)
                ctrl_reg[wr_counter]    <= s_axi_wdata;
            else if (wr_reg == REG_MAX_CYCLES)
                max_cycles[wr_counter]  <= s_axi_wdata;
            else if (wr_reg == REG_MAX_TIME)
                max_time[wr_counter]    <= s_axi_wdata;
            else if (wr_reg == REG_TIMEOUT)
                timeout_reg[wr_counter] <= s_axi_wdata;
        end
    end

    //=========================================================================
    // AXI Write Channel
    //=========================================================================
    localparam WR_IDLE = 2'b00;
    localparam WR_DATA = 2'b01;
    localparam WR_RESP = 2'b10;

    reg [1:0] wr_state;
    reg [AXI_ADDR_WIDTH-1:0] wr_addr;
    wire [1:0] wr_counter = wr_addr[6:5];  // Counter index (bits 6:5 for 32-byte blocks)
    wire [2:0] wr_reg     = wr_addr[4:2];  // Register within counter
    reg wr_en;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            wr_state <= WR_IDLE;
            wr_addr <= 0;
            s_axi_awready <= 1'b0;
            s_axi_wready <= 1'b0;
            s_axi_bvalid <= 1'b0;
            s_axi_bresp <= 2'b00;
            wr_en <= 1'b0;
        end else begin
            wr_en <= 1'b0;
            case (wr_state)
                WR_IDLE: begin
                    s_axi_bvalid <= 1'b0;
                    if (s_axi_awvalid && s_axi_wvalid) begin
                        s_axi_awready <= 1'b1;
                        s_axi_wready <= 1'b1;
                        wr_addr <= s_axi_awaddr;
                        wr_en <= 1'b1;
                        wr_state <= WR_RESP;
                    end else if (s_axi_awvalid) begin
                        s_axi_awready <= 1'b1;
                        wr_addr <= s_axi_awaddr;
                        wr_state <= WR_DATA;
                    end
                end

                WR_DATA: begin
                    s_axi_awready <= 1'b0;
                    if (s_axi_wvalid) begin
                        s_axi_wready <= 1'b1;
                        wr_en <= 1'b1;
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
    // AXI Read Channel
    //=========================================================================
    localparam RD_IDLE = 2'b00;
    localparam RD_DATA = 2'b01;

    reg [1:0] rd_state;
    reg [AXI_ADDR_WIDTH-1:0] rd_addr;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            rd_state <= RD_IDLE;
            rd_addr <= 0;
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

                    // Address decode
                    if (rd_addr[7:2] == 6'h1F) begin
                        // VERSION register at 0x7C
                        s_axi_rdata <= `FBC_VERSION;
                    end else begin
                        // Counter registers
                        case (rd_addr[4:2])
                            REG_CTRL:       s_axi_rdata <= ctrl_reg[rd_addr[6:5]];
                            REG_STATUS:     s_axi_rdata <= status_reg[rd_addr[6:5]];
                            REG_MAX_CYCLES: s_axi_rdata <= max_cycles[rd_addr[6:5]];
                            REG_MAX_TIME:   s_axi_rdata <= max_time[rd_addr[6:5]];
                            REG_CYCLES:     s_axi_rdata <= cycle_count[rd_addr[6:5]];
                            REG_TIME:       s_axi_rdata <= time_count[rd_addr[6:5]];
                            REG_TIMEOUT:    s_axi_rdata <= timeout_reg[rd_addr[6:5]];
                            default:        s_axi_rdata <= 32'hDEAD_BEEF;
                        endcase
                    end

                    if (s_axi_rready && s_axi_rvalid) begin
                        s_axi_rvalid <= 1'b0;
                        rd_state <= RD_IDLE;
                    end
                end

                default: rd_state <= RD_IDLE;
            endcase
        end
    end

    //=========================================================================
    // Frequency Counter Instances
    //=========================================================================
    genvar n;
    generate
        for (n = 0; n < NUM_COUNTERS; n = n + 1) begin : freq_counters
            freq_counter_core u_freq_counter (
                .clk            (clk),
                .resetn         (resetn),

                // Inputs to measure
                .all_inputs     (all_inputs),

                // Control
                .ctrl           (ctrl_reg[n]),
                .max_cycles     (max_cycles[n]),
                .max_time       (max_time[n]),
                .timeout        (timeout_reg[n]),

                // Outputs
                .status         (status_reg[n]),
                .cycle_count    (cycle_count[n]),
                .time_count     (time_count[n]),
                .irq            (counter_irq[n])
            );
        end
    endgenerate

    // Combined IRQ output
    assign irq = |{counter_irq[3], counter_irq[2], counter_irq[1], counter_irq[0]};

endmodule


//=============================================================================
// Frequency Counter Core (single channel)
//=============================================================================
module freq_counter_core (
    input wire clk,
    input wire resetn,

    // Signal inputs
    input wire [`PIN_COUNT-1:0] all_inputs,

    // Control from registers
    input wire [31:0] ctrl,
    input wire [31:0] max_cycles,
    input wire [31:0] max_time,
    input wire [31:0] timeout,

    // Status outputs
    output wire [31:0] status,
    output reg  [31:0] cycle_count,
    output reg  [31:0] time_count,
    output wire        irq
);

    //=========================================================================
    // State Machine
    //=========================================================================
    localparam FREQ_IDLE = 4'd1;
    localparam FREQ_WAIT = 4'd2;
    localparam FREQ_RUN  = 4'd4;
    localparam FREQ_DONE = 4'd8;

    reg [3:0] state;

    //=========================================================================
    // Control Register Decode
    //=========================================================================
    wire enable     = ctrl[0];
    wire irq_en     = ctrl[1];
    wire [7:0] sig_sel  = ctrl[15:8];
    wire [7:0] trig_sel = ctrl[23:16];

    // Shadow registers (latched at IDLE->WAIT transition)
    reg [7:0] sig_sel_reg;
    reg [7:0] trig_sel_reg;
    reg       irq_en_reg;
    reg [31:0] max_cycles_reg;
    reg [31:0] max_time_reg;

    //=========================================================================
    // Signal Selection and Edge Detection
    //=========================================================================
    wire sig_net  = (sig_sel_reg < `PIN_COUNT)  ? all_inputs[sig_sel_reg]  : 1'b0;
    wire trig_net = (trig_sel_reg < `PIN_COUNT) ? all_inputs[trig_sel_reg] : 1'b0;

    // Synchronizers for metastability
    reg [1:0] sig_sync;
    reg [1:0] trig_sync;

    always @(posedge clk) begin
        sig_sync  <= {sig_sync[0], sig_net};
        trig_sync <= {trig_sync[0], trig_net};
    end

    wire sig_posedge  = sig_sync[0]  & ~sig_sync[1];
    wire trig_posedge = trig_sync[0] & ~trig_sync[1];

    //=========================================================================
    // Timeout Counter
    //=========================================================================
    reg [31:0] timeout_count;
    reg timeout_err;
    wire timeout_reached = (timeout_count >= timeout);

    wire reset_timeout = ~enable |
                        ((state == FREQ_IDLE) && (next_state == FREQ_WAIT)) |
                        ((state == FREQ_WAIT) && (next_state == FREQ_RUN));

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            timeout_count <= 0;
        end else if (reset_timeout) begin
            timeout_count <= 0;
        end else if ((state == FREQ_WAIT) || (state == FREQ_IDLE)) begin
            if (timeout_count != 32'hffffffff)
                timeout_count <= timeout_count + 1;
        end
    end

    always @(posedge clk or negedge resetn) begin
        if (!resetn)
            timeout_err <= 0;
        else if (state == FREQ_IDLE)
            timeout_err <= 0;
        else if ((state == FREQ_WAIT) && timeout_reached)
            timeout_err <= 1;
    end

    //=========================================================================
    // State Machine
    //=========================================================================
    wire done    = (state == FREQ_DONE);
    wire idle    = (state == FREQ_IDLE);
    wire running = (state == FREQ_RUN);
    wire waiting = (state == FREQ_WAIT);

    wire last_time   = (time_count == max_time_reg - 1);
    wire last_cycle  = (cycle_count == max_cycles_reg - 1);

    wire [3:0] next_state;
    assign next_state =
        (~enable)                                         ? FREQ_IDLE :
        ((state == FREQ_IDLE) && (trig_sel_reg == 8'hfe)) ? FREQ_WAIT :  // Immediate trigger
        ((state == FREQ_IDLE) && trig_posedge)            ? FREQ_WAIT :
        ((state == FREQ_IDLE) && timeout_reached)         ? FREQ_DONE :
        ((state == FREQ_WAIT) && sig_posedge)             ? FREQ_RUN  :
        ((state == FREQ_WAIT) && timeout_reached)         ? FREQ_DONE :
        ((state == FREQ_RUN)  && last_time)               ? FREQ_DONE :
        ((state == FREQ_RUN)  && last_cycle && sig_posedge) ? FREQ_DONE :
        ((state == FREQ_DONE) && ~enable)                 ? FREQ_IDLE :
        state;

    always @(posedge clk or negedge resetn) begin
        if (!resetn)
            state <= FREQ_IDLE;
        else
            state <= next_state;
    end

    //=========================================================================
    // Shadow Registers (latched when entering measurement)
    //=========================================================================
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            sig_sel_reg     <= 8'hff;
            trig_sel_reg    <= 8'hff;
            irq_en_reg      <= 0;
            max_time_reg    <= 32'hffffffff;
            max_cycles_reg  <= 32'h1;
        end else if (state == FREQ_IDLE) begin
            sig_sel_reg     <= sig_sel;
            trig_sel_reg    <= trig_sel;
            irq_en_reg      <= irq_en;
            max_time_reg    <= (max_time == 0)   ? 32'h1 : max_time;
            max_cycles_reg  <= (max_cycles == 0) ? 32'h1 : max_cycles;
        end
    end

    //=========================================================================
    // Measurement Counters
    //=========================================================================
    // Time counter (increments every clock while running)
    always @(posedge clk or negedge resetn) begin
        if (!resetn)
            time_count <= 0;
        else if (state == FREQ_IDLE)
            time_count <= 0;
        else if (running)
            time_count <= time_count + 1;
    end

    // Cycle counter (increments on signal posedge while running)
    always @(posedge clk or negedge resetn) begin
        if (!resetn)
            cycle_count <= 0;
        else if (state == FREQ_IDLE)
            cycle_count <= 0;
        else if (running && sig_posedge)
            cycle_count <= cycle_count + 1;
    end

    //=========================================================================
    // Status Register and IRQ
    //=========================================================================
    assign status[0]     = done;
    assign status[1]     = idle;
    assign status[2]     = waiting;
    assign status[3]     = running;
    assign status[4]     = irq_en_reg;
    assign status[5]     = ((time_count == 0) || (cycle_count == 0)) && done;  // Invalid measurement
    assign status[6]     = timeout_err;
    assign status[7]     = timeout_reached;
    assign status[15:8]  = sig_sel_reg;
    assign status[23:16] = trig_sel_reg;
    assign status[31:24] = 8'h0;

    assign irq = done && irq_en_reg;

endmodule
