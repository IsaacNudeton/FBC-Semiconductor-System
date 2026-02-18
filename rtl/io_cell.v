`timescale 1ns / 1ps
//=============================================================================
// I/O Cell - Single Pin Handler
//=============================================================================
//
// Based on 2016 reference: reference/kzhang_v2_2016/io_table.v (single_pin)
//
// Handles all pin types:
//   BIDI (0):     Bidirectional with compare
//   INPUT (1):    Input only, compare enabled
//   OUTPUT (2):   Output only, no compare
//   OPEN_C (3):   Open collector
//   PULSE (4):    Pulse output with programmable timing
//   NPULSE (5):   Inverted pulse output
//   RESERVED (6): Not implemented - falls through to BIDI
//   VEC_CLK (7):  Clock output with timing
//
// Note: PIN_TYPE_ERR_TRIG (0x6) and PIN_TYPE_VEC_CLK_EN (0x8) from fbc_pkg.vh
// are NOT implemented here. The 2016 reference notes these "cause serious
// timing problems". They default to BIDI behavior if used.
//
// Pulse timing is controlled by pulse_ctrl_bits:
//   [15:8] = start time (match with vec_clk_cnt to start pulse)
//   [7:0]  = end time (match with vec_clk_cnt to end pulse)
//
//=============================================================================

`include "fbc_pkg.vh"

module io_cell #(
    parameter FAST_MODE = 0  // 0=normal (2-stage pipe), 1=fast (0-stage, combinational)
)(
    input wire        clk,              // Fast clock (for timing)
    input wire        resetn,

    //=========================================================================
    // Pin Type Configuration
    //=========================================================================
    input wire [3:0]  pin_type,         // Pin type code
    input wire [15:0] pulse_ctrl_bits,  // [15:8]=start, [7:0]=end timing

    //=========================================================================
    // Timing
    //=========================================================================
    input wire [7:0]  vec_clk_cnt,      // Counter within vector clock period

    //=========================================================================
    // Vector Data Input
    //=========================================================================
    input wire        dout,             // Data to output
    input wire        oen,              // Output enable (0=drive, 1=tristate)

    //=========================================================================
    // Physical Pin Interface
    //=========================================================================
    input wire        pin_din,          // Input from physical pin
    output wire       pin_dout,         // Output to physical pin
    output wire       pin_oen,          // Tristate control (0=drive, 1=tristate)

    //=========================================================================
    // Error Output
    //=========================================================================
    output wire       error             // Comparison error detected
);

    //=========================================================================
    // Pipeline Configuration
    //=========================================================================
    // Normal mode: 2-stage pipeline for timing closure
    // Fast mode: Combinational for minimum latency (use for triggers/clocks)
    //=========================================================================

    // Internal signals from pin type logic
    reg dout_next, oen_next, error_next;

    //=========================================================================
    // Pipeline Implementation
    //=========================================================================
    // Note: Using simple delay chain instead of generate for parser compat.
    // FAST_MODE parameter is checked at synthesis time.
    //=========================================================================

    // Pipeline stage 1
    reg dout_p1, oen_p1, error_p1;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            dout_p1  <= 1'b0;
            oen_p1   <= 1'b1;
            error_p1 <= 1'b0;
        end else begin
            dout_p1  <= dout_next;
            oen_p1   <= oen_next;
            error_p1 <= error_next;
        end
    end

    // Pipeline stage 2 (only used when FAST_MODE=0)
    reg dout_p2, oen_p2, error_p2;

    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            dout_p2  <= 1'b0;
            oen_p2   <= 1'b1;
            error_p2 <= 1'b0;
        end else begin
            dout_p2  <= dout_p1;
            oen_p2   <= oen_p1;
            error_p2 <= error_p1;
        end
    end

    // Output selection based on FAST_MODE
    // FAST_MODE=0: 2-stage pipeline (BIM pins, timing critical)
    // FAST_MODE=1: 1-stage pipeline (fast pins, low latency)
    assign pin_dout = (FAST_MODE == 0) ? dout_p2  : dout_p1;
    assign pin_oen  = (FAST_MODE == 0) ? oen_p2   : oen_p1;
    assign error    = (FAST_MODE == 0) ? error_p2 : error_p1;

    //=========================================================================
    // Pin Type Logic (combinational - feeds pipeline)
    //=========================================================================
    // Extract pulse timing fields using shift (parser limitation workaround)
    wire [15:0] pulse_shifted;
    assign pulse_shifted = pulse_ctrl_bits >> 8;

    wire [7:0] pulse_start;
    wire [7:0] pulse_end;
    assign pulse_start = pulse_shifted;   // Upper 8 bits (start time)
    assign pulse_end   = pulse_ctrl_bits; // Lower 8 bits (end time) - implicit truncation

    // Pulse state tracking
    reg pulse_state;
    always @(posedge clk or negedge resetn) begin
        if (!resetn)
            pulse_state <= 1'b0;
        else if (pulse_start == vec_clk_cnt)
            pulse_state <= 1'b1;
        else if (pulse_end == vec_clk_cnt)
            pulse_state <= 1'b0;
    end

    always @(*) begin
        // Defaults
        dout_next  = 1'b0;
        oen_next   = 1'b1;  // Tristate
        error_next = 1'b0;

        case (pin_type)

            //=============================================================
            // INPUT (0x1): Always input, compare H/L
            //=============================================================
            `PIN_TYPE_INPUT: begin
                oen_next   = 1'b1;
                dout_next  = dout;
                error_next = oen & (pin_din ^ dout);
            end

            //=============================================================
            // OUTPUT (0x2): Always output, no compare
            //=============================================================
            `PIN_TYPE_OUTPUT: begin
                oen_next  = oen;
                dout_next = dout;
            end

            //=============================================================
            // OPEN_C (0x3): Open collector
            //=============================================================
            `PIN_TYPE_OPEN_C: begin
                oen_next   = (dout | oen);
                dout_next  = 1'b0;
                error_next = oen & (dout ^ pin_din);
            end

            //=============================================================
            // PULSE (0x4): Pulse output with programmable timing
            //=============================================================
            `PIN_TYPE_PULSE: begin
                case ({dout, oen})
                    2'b00: begin
                        oen_next  = 1'b0;
                        dout_next = 1'b0;
                    end
                    2'b10: begin
                        oen_next  = 1'b0;
                        dout_next = 1'b1;
                    end
                    2'b01: begin
                        oen_next  = 1'b0;
                        dout_next = pulse_state;
                    end
                    default: begin
                        oen_next  = 1'b1;
                        dout_next = 1'b0;
                    end
                endcase
            end

            //=============================================================
            // NPULSE (0x5): Inverted pulse output
            //=============================================================
            `PIN_TYPE_NPULSE: begin
                case ({dout, oen})
                    2'b00: begin
                        oen_next  = 1'b0;
                        dout_next = 1'b0;
                    end
                    2'b10: begin
                        oen_next  = 1'b0;
                        dout_next = 1'b1;
                    end
                    2'b01: begin
                        oen_next  = 1'b0;
                        dout_next = ~pulse_state;  // Inverted
                    end
                    default: begin
                        oen_next  = 1'b1;
                        dout_next = 1'b0;
                    end
                endcase
            end

            //=============================================================
            // VEC_CLK (0x7): Vector clock output
            //=============================================================
            `PIN_TYPE_VEC_CLK: begin
                oen_next  = 1'b0;
                dout_next = pulse_state;
            end

            //=============================================================
            // VEC_CLK_EN (0x8): Vector clock enable output
            //=============================================================
            `PIN_TYPE_VEC_CLK_EN: begin
                oen_next  = 1'b0;
                dout_next = oen;  // Use oen as enable signal
            end

            //=============================================================
            // BIDI (0x0) and default: Bidirectional
            //=============================================================
            default: begin
                oen_next   = oen;
                dout_next  = dout;
                error_next = oen & (pin_din ^ dout);
            end

        endcase
    end

endmodule
