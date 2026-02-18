`ifndef __VECTOR_H__
`define __VECTOR_H__

`define FPGA_VERSION 32'h0916_010F // month-year-main-minor

`define PIN_COUNT    160 //158 // number of actual pins (not counting clocks)

`define VECTOR_WIDTH 128 
`define REPEAT_WIDTH  32 

`define M_STREAM_WIDTH      `VECTOR_WIDTH*2 + `REPEAT_WIDTH

`define EXTRA_GPIO_WIDTH    `PIN_COUNT-`VECTOR_WIDTH

`define DMA_MAX_BURST       (256/2)
`define FIFO_LENGTH         (2*`DMA_MAX_BURST) 
`define FIFO_SYNC_STAGES    2 // 

//`define LOGIC_ANALYZER_FIFO_LENGTH   32
//`define LOGIC_ANALYZER_MAX_BURST     16

`define BRAM_MEM_SIZE       1024      // 1024x128 
`define MAX_ERROR_COUNT     (`BRAM_MEM_SIZE) // 
`define ERROR_COUNT_WIDTH   32 // 10 // 

`define HOLD_TIME 1 //  ns only used for simulation 

`define PIN_TYPE_WIDTH 4 // 4 but pintype 
//------------------------------------------------------------------------------------------------
// PIN_TYPES definitions 
 
`define BIDI_PIN        4'b0000 // (reset/default type) {1,0, H,L}  error = (oen)(pin_din ^ dout) 
`define INPUT_PIN       4'b0001 // {X,X, H,L }                      error = (same as bidi) 
`define OUTPUT_PIN      4'b0010 // {1,0, Z  Z} (same as bidi pin) but oen is always 0 and error =0  
`define OPEN_C_PIN      4'b0011 // (1,0, X, L} pin_dout = 0, pin_oen = dout; error = oen&(dout==0)&(pin_din!=0) 

`define PULSE_PIN       4'b0100 // {P}  (dout_pin = {posedge(T/4), negedge(3T/4)} 
`define NPULSE_PIN      4'b0101 // {N}, (dout_pin = ~(PULSE_PIN) 

`define ERROR_TRIG      4'b0110 // dout_pin = error_detected (debug : scope trigger ) 
`define VEC_CLK_PIN     4'b0111 // (dout_pin = vec_clk) 
`define VEC_CLK_EN_PIN  4'b1000 // 


`endif 