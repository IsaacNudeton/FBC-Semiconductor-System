//=============================================================================
// AXI Device DNA Reader (0x400A_0000)
//=============================================================================
//
// Reads the 57-bit unique Device DNA from the Xilinx 7-series DNA_PORT
// primitive and exposes it via AXI-Lite registers.
//
// Register map (read-only):
//   0x00: DNA_LO    — DNA[31:0]
//   0x04: DNA_HI    — {7'b0, DNA[56:32]}
//   0x08: DNA_STATUS — {31'b0, dna_valid}
//
// The DNA is read automatically after reset. dna_valid asserts once the
// 57-bit shift sequence completes (~57 clocks after reset release).
// All registers return 0 until dna_valid is set.
//
// DNA_PORT (7-series) — NOT DNA_PORTE2 (UltraScale only).
//

module axi_device_dna (
    input  wire        clk,
    input  wire        rst_n,

    // AXI-Lite Read Interface
    input  wire [11:0] s_axi_araddr,
    input  wire        s_axi_arvalid,
    output wire        s_axi_arready,
    output reg  [31:0] s_axi_rdata,
    output wire [1:0]  s_axi_rresp,
    output reg         s_axi_rvalid,
    input  wire        s_axi_rready,

    // AXI-Lite Write Interface (accept + ignore — this peripheral is read-only)
    input  wire [11:0] s_axi_awaddr,
    input  wire        s_axi_awvalid,
    output wire        s_axi_awready,
    input  wire [31:0] s_axi_wdata,
    input  wire [3:0]  s_axi_wstrb,
    input  wire        s_axi_wvalid,
    output wire        s_axi_wready,
    output wire [1:0]  s_axi_bresp,
    output wire        s_axi_bvalid,
    input  wire        s_axi_bready
);

    //=========================================================================
    // DNA_PORT Primitive + Shift FSM
    //=========================================================================

    wire dna_dout;
    reg  dna_read;
    reg  dna_shift;

    DNA_PORT #(
        .SIM_DNA_VALUE(57'h1_DEAD_BEEF_CAFE_42)  // Recognizable in simulation
    ) dna_port_inst (
        .DOUT  (dna_dout),
        .CLK   (clk),
        .DIN   (1'b0),        // No recirculation — read once
        .READ  (dna_read),
        .SHIFT (dna_shift)
    );

    // FSM states
    localparam S_IDLE  = 2'd0;
    localparam S_READ  = 2'd1;
    localparam S_SHIFT = 2'd2;
    localparam S_DONE  = 2'd3;

    reg [1:0]  state;
    reg [5:0]  shift_cnt;
    reg [56:0] dna_shift_reg;

    // Latched output registers (read by AXI)
    reg [31:0] dna_lo;
    reg [31:0] dna_hi;
    reg        dna_valid;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state         <= S_IDLE;
            shift_cnt     <= 6'd0;
            dna_shift_reg <= 57'd0;
            dna_read      <= 1'b0;
            dna_shift     <= 1'b0;
            dna_lo        <= 32'd0;
            dna_hi        <= 32'd0;
            dna_valid     <= 1'b0;
        end else begin
            case (state)
                S_IDLE: begin
                    // Issue READ pulse to parallel-load DNA into shift register
                    dna_read  <= 1'b1;
                    dna_shift <= 1'b0;
                    state     <= S_READ;
                end

                S_READ: begin
                    // READ was asserted last cycle — DNA is loaded, MSB on DOUT
                    dna_read  <= 1'b0;
                    dna_shift <= 1'b1;
                    dna_shift_reg <= {dna_shift_reg[55:0], dna_dout};  // Capture bit 56 (MSB)
                    shift_cnt <= 6'd1;
                    state     <= S_SHIFT;
                end

                S_SHIFT: begin
                    dna_shift_reg <= {dna_shift_reg[55:0], dna_dout};
                    shift_cnt <= shift_cnt + 6'd1;
                    if (shift_cnt == 6'd56) begin
                        // All 57 bits captured
                        dna_shift <= 1'b0;
                        state     <= S_DONE;
                    end
                end

                S_DONE: begin
                    // Latch into AXI-readable registers
                    // dna_shift_reg[56] = MSB (first bit out), [0] = LSB (last bit out)
                    dna_lo    <= dna_shift_reg[31:0];
                    dna_hi    <= {7'd0, dna_shift_reg[56:32]};
                    dna_valid <= 1'b1;
                    // Stay in DONE forever — DNA is immutable
                end
            endcase
        end
    end

    //=========================================================================
    // AXI-Lite Read Interface
    //=========================================================================

    assign s_axi_arready = !s_axi_rvalid;
    assign s_axi_rresp   = 2'b00;  // OKAY

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            s_axi_rvalid <= 1'b0;
            s_axi_rdata  <= 32'd0;
        end else begin
            if (s_axi_rvalid && s_axi_rready) begin
                s_axi_rvalid <= 1'b0;
            end
            if (s_axi_arvalid && !s_axi_rvalid) begin
                s_axi_rvalid <= 1'b1;
                case (s_axi_araddr[3:0])
                    4'h0: s_axi_rdata <= dna_lo;               // 0x00: DNA[31:0]
                    4'h4: s_axi_rdata <= dna_hi;               // 0x04: DNA[56:32]
                    4'h8: s_axi_rdata <= {31'd0, dna_valid};   // 0x08: Valid flag
                    default: s_axi_rdata <= 32'd0;
                endcase
            end
        end
    end

    //=========================================================================
    // AXI-Lite Write Interface (accept + discard — read-only peripheral)
    //=========================================================================

    assign s_axi_awready = 1'b1;
    assign s_axi_wready  = 1'b1;
    assign s_axi_bresp   = 2'b00;
    assign s_axi_bvalid  = s_axi_wvalid;

endmodule
