`timescale 1ns / 1ps
//=============================================================================
// FBC Decoder - Hardware FBC Bytecode Execution Engine
//=============================================================================
//
// Decodes FBC (FORCE Bytecode) instructions and generates pin vectors.
// Replaces raw vector DMA with compressed instruction stream.
//
// Compression ratios: 1000:1 to 20000:1 typical
//
//=============================================================================

`include "fbc_pkg.vh"

module fbc_decoder #(
    parameter VECTOR_WIDTH = `VECTOR_WIDTH,
    parameter REPEAT_WIDTH = `REPEAT_WIDTH
)(
    input wire clk,
    input wire resetn,

    //=========================================================================
    // FBC Instruction Input (from DMA/AXI Stream)
    //=========================================================================
    input wire [63:0]  fbc_instr,       // Instruction word
    input wire [127:0] fbc_payload,     // Extended payload (for SET_PINS etc)
    input wire         fbc_valid,       // Instruction valid
    output wire        fbc_ready,       // Ready for next instruction

    //=========================================================================
    // Vector Output (to pin driver)
    //=========================================================================
    output reg [VECTOR_WIDTH-1:0] vec_dout,      // Pin output values
    output reg [VECTOR_WIDTH-1:0] vec_oen,       // Output enables (active low)
    output reg [REPEAT_WIDTH-1:0] vec_repeat,    // Repeat count
    output reg                    vec_valid,     // Vector valid
    input wire                    vec_ready,     // Downstream ready

    //=========================================================================
    // Status
    //=========================================================================
    output reg         running,         // Decoder is executing
    output reg         done,            // Program complete (HALT seen)
    output reg         error,           // Decode error
    output reg [31:0]  instr_count,     // Instructions executed
    output reg [63:0]  cycle_count      // Total cycles generated
);

    //=========================================================================
    // Instruction decode
    //=========================================================================
    wire [7:0]  opcode  = fbc_instr[63:56];
    wire [7:0]  flags   = fbc_instr[55:48];
    wire [47:0] operand = fbc_instr[47:0];

    wire is_last = flags[0];  // FBC_FLAG_LAST
    wire gen_irq = flags[1];  // FBC_FLAG_IRQ

    //=========================================================================
    // State machine
    //=========================================================================
    localparam S_IDLE       = 4'd0;
    localparam S_FETCH      = 4'd1;
    localparam S_DECODE     = 4'd2;
    localparam S_EXECUTE    = 4'd3;
    localparam S_WAIT_OUT   = 4'd4;
    localparam S_LOOP       = 4'd5;
    localparam S_DONE       = 4'd6;
    localparam S_ERROR      = 4'd7;

    reg [3:0] state, next_state;

    //=========================================================================
    // Registers
    //=========================================================================
    reg [VECTOR_WIDTH-1:0] current_dout;    // Current pin values
    reg [VECTOR_WIDTH-1:0] current_oen;     // Current output enables
    reg [31:0]             loop_count;      // Loop counter
    reg [31:0]             loop_target;     // Loop target count
    reg [31:0]             wait_count;      // Wait cycle counter

    //=========================================================================
    // State machine - next state logic
    //=========================================================================
    always @(*) begin
        next_state = state;

        case (state)
            S_IDLE: begin
                if (fbc_valid && resetn)
                    next_state = S_DECODE;
            end

            S_DECODE: begin
                case (opcode)
                    `FBC_NOP:         next_state = S_IDLE;
                    `FBC_HALT:        next_state = S_DONE;
                    `FBC_SET_PINS:    next_state = S_EXECUTE;
                    `FBC_SET_OEN:     next_state = S_EXECUTE;
                    // SET_BOTH requires 256-bit payload (dout+oen) but bus is 128-bit.
                    // Use SET_PINS + SET_OEN separately. Re-enable when bus is widened.
                    `FBC_SET_BOTH:    next_state = S_ERROR;
                    `FBC_PATTERN_REP: next_state = S_EXECUTE;
                    `FBC_WAIT:        next_state = S_EXECUTE;
                    `FBC_LOOP_N:      next_state = S_LOOP;
                    default:          next_state = S_ERROR;
                endcase
            end

            S_EXECUTE: begin
                case (opcode)
                    `FBC_SET_PINS, `FBC_SET_OEN, `FBC_SET_BOTH:
                        next_state = S_WAIT_OUT;
                    `FBC_PATTERN_REP:
                        next_state = S_WAIT_OUT;
                    `FBC_WAIT:
                        next_state = (wait_count == 0) ? S_IDLE : S_EXECUTE;
                    default:
                        next_state = S_IDLE;
                endcase
            end

            S_WAIT_OUT: begin
                if (vec_ready)
                    next_state = S_IDLE;
            end

            // NOTE: FBC_LOOP_N is currently incomplete. This state machine
            // correctly counts iterations, but there is no instruction buffer
            // or program counter to replay the loop body. Full loop support
            // requires adding an instruction FIFO or PC-based replay.
            // For now, loops will count but not re-execute instructions.
            S_LOOP: begin
                if (loop_count >= loop_target)
                    next_state = S_IDLE;
                else
                    next_state = S_LOOP;  // Stay in loop, continue counting
            end

            S_DONE: begin
                // Stay in done until reset
                next_state = S_DONE;
            end

            S_ERROR: begin
                // Stay in error until reset
                next_state = S_ERROR;
            end

            default: next_state = S_IDLE;
        endcase
    end

    //=========================================================================
    // State machine - registered
    //=========================================================================
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            state <= S_IDLE;
        end else begin
            state <= next_state;
        end
    end

    //=========================================================================
    // Execution logic
    //=========================================================================
    always @(posedge clk or negedge resetn) begin
        if (!resetn) begin
            current_dout <= {VECTOR_WIDTH{1'b0}};
            current_oen  <= {VECTOR_WIDTH{1'b1}};  // All inputs by default
            vec_dout     <= {VECTOR_WIDTH{1'b0}};
            vec_oen      <= {VECTOR_WIDTH{1'b1}};
            vec_repeat   <= 32'd1;
            vec_valid    <= 1'b0;
            loop_count   <= 32'd0;
            loop_target  <= 32'd0;
            wait_count   <= 32'd0;
            running      <= 1'b0;
            done         <= 1'b0;
            error        <= 1'b0;
            instr_count  <= 32'd0;
            cycle_count  <= 64'd0;
        end else begin
            // Default: clear valid when accepted
            if (vec_valid && vec_ready) begin
                vec_valid <= 1'b0;
            end

            case (state)
                S_IDLE: begin
                    running <= fbc_valid;
                end

                S_DECODE: begin
                    running <= 1'b1;
                    instr_count <= instr_count + 1;

                    // Pre-load wait counter for WAIT instruction
                    if (opcode == `FBC_WAIT) begin
                        wait_count <= operand[31:0];
                    end

                    // Pre-load loop target for LOOP_N
                    if (opcode == `FBC_LOOP_N) begin
                        loop_target <= operand[31:0];
                        loop_count <= 32'd0;
                    end
                end

                S_EXECUTE: begin
                    case (opcode)
                        `FBC_SET_PINS: begin
                            current_dout <= fbc_payload[VECTOR_WIDTH-1:0];
                            vec_dout <= fbc_payload[VECTOR_WIDTH-1:0];
                            vec_oen <= current_oen;
                            vec_repeat <= 32'd1;
                            vec_valid <= 1'b1;
                            cycle_count <= cycle_count + 1;
                        end

                        `FBC_SET_OEN: begin
                            current_oen <= fbc_payload[VECTOR_WIDTH-1:0];
                            vec_dout <= current_dout;
                            vec_oen <= fbc_payload[VECTOR_WIDTH-1:0];
                            vec_repeat <= 32'd1;
                            vec_valid <= 1'b1;
                            cycle_count <= cycle_count + 1;
                        end

                        // FBC_SET_BOTH removed — requires 256-bit payload,
                        // bus only carries 128. Use SET_PINS + SET_OEN instead.

                        `FBC_PATTERN_REP: begin
                            // Repeat current pattern N times
                            vec_dout <= current_dout;
                            vec_oen <= current_oen;
                            vec_repeat <= operand[31:0];
                            vec_valid <= 1'b1;
                            cycle_count <= cycle_count + operand[31:0];
                        end

                        `FBC_WAIT: begin
                            if (wait_count > 0) begin
                                wait_count <= wait_count - 1;
                            end
                        end
                    endcase
                end

                S_LOOP: begin
                    loop_count <= loop_count + 1;
                end

                S_DONE: begin
                    running <= 1'b0;
                    done <= 1'b1;
                end

                S_ERROR: begin
                    running <= 1'b0;
                    error <= 1'b1;
                end
            endcase
        end
    end

    //=========================================================================
    // Ready signal - can accept new instruction
    //=========================================================================
    assign fbc_ready = (state == S_IDLE) ||
                       (state == S_WAIT_OUT && vec_ready);

endmodule
