
`timescale 1 ns / 1 ps

`include "vector.vh" 

`define FREQ_AXI4_LITE  // don't need burst transfers 
 
// each counter has 6 u32 registers 
// maximum number of counters = 256
`define NUM_COUNTERS 8 
`define FREQ_AXI_ADDR_WIDTH 14 // 4KByre block of u32 space 
`define FREQ_MEM_ADDR_WIDTH 10  // 16 u32 addresses => 4 bit mem; 
`define FREQ_MEM_DATA_WIDTH 32 

`define FREQ_NUM_U32_REGS  (`NUM_COUNTERS*8)//(1<<`FREQ_MEM_ADDR_WIDTH) //(`CONTROL_BIT_WIDTH/32)  // 32*16 = 512 bits
//`define CONTROL_BIT_WIDTH (`FREQ_MEM_DATA_WIDTH*`FREQ_NUM_U32_REGS) // must be u32 aligned (n*32); n==1..1024 

`include "axi_slave.vh" 

module axi_freq_counter ( 
	all_inputs, irq, 
	awaddr, awvalid, awready, awlen, awsize, awburst,
	araddr, arvalid, arready, arlen, arsize, arburst,
	rdata, rresp, rvalid, rready, rlast, // rid, ruser 
	wdata, wstrb, wvalid, wready, wlast, // wid, wuser 
	bresp, bvalid, bready,  
	clk0, resetn 
  ) ; 
	input clk0 ; 
	input resetn; 
	input  [`PIN_COUNT-1:0] all_inputs ; 
	output irq; 

	// write address channel 
	input  [`FREQ_AXI_ADDR_WIDTH-1:0] awaddr ; 
	input        awvalid; 
    output       awready ;     
	input  [7:0] awlen ; 
	input  [2:0] awsize ; 
	input  [1:0] awburst ; 
//`ifndef AXI_PROTECTION
//	input [1:0] awlock;  // lock Type [???} 
//	input [3:0] awcache; // Memory Access Type identiver  
//	input [2:0] awprot;  // {Ins(1)/Data(0)[0], non-secure[2], Priviledge_access[0],,   
//	input [3:0] awqos; // quality of service transation attribute ???  
//	input [`REGION_WIDTH-1:0] awregion; // memory Regionattribute  
//	input [`ID_WIDTH-1:0] awid; // waddr ID (transaciotn order)   
//	input [`USER_WIDTH-1:0] awuser;   //  User Defined Extra 
// `endif 

	// read Address channel 
	input  [`FREQ_AXI_ADDR_WIDTH-1:0] araddr ; 
	input  arvalid; 
	output arready ; 
	input [7:0] arlen;   // num of transfers per burst (per awaddr)  
	input [2:0] arsize;  // buret bytes size {1,2,4, 8, 16, 32, 64, 128}   
	input [1:0] arburst; // burst type {FIXED, INCR, WRAP, RESERVED } 
// `ifdef AXI_SECURITY  
//	input [1:0] arlock;  // lock Type [???} 
//	input [3:0] arcache; // Memory Access Type identiver  
//	input [2:0] arprot;  // {Ins(1)/Data(0)[0], non-secure[2], Priviledge_access[0],,   
//	input [3:0] arqos; // quality of service transation attribute ???  
//	input [`REGION_WIDTH-1:0] arregion; // memory Region attribute  
// `endif 
// `ifdef AXI_TRANS_ORDERING // interconnects  
//	input [`ID_WIDTH-1:0]     arid;    // waddr ID (transaciotn oreder)   
//	input [`USER_WIDTH-1:0]   aruser;  // waddr User control  
//`endif 

	// read data channel (to master ) 
	output [31:0] rdata ; 
	output  [1:0] rresp ; 
	output        rvalid; 
	input 	      rready ; 
	output        rlast ; 
	//output  [`ID_WIDTH-1:0]  rid; 
	//output  [`USER_WIDTH-1:0]  ruser; 

	// write data channel (from master ) 
	input  [31:0] wdata ; 
	input   [3:0] wstrb ; // byte mask 
	input         wvalid; 
	output 	      wready ; 
	input         wlast ;  // end od burst 
	//output  [`ID_WIDTH-1:0]  wid; 
	//output  [`USER_WIDTH-1:0]  wuser; 

	// write response channel 
	output [1:0] bresp; 
	output       bvalid; 
	input        bready; 
//	output [`ID_WIDTH-1n:0] bid  ;  // ID tag
//	output [`USER_WIDTH-1n:0] buser;  // ID tag

    (* mark_debug="false" *)wire [`FREQ_AXI_ADDR_WIDTH-1:0] u8_raddr; 
    (* mark_debug="false" *)wire [`FREQ_AXI_ADDR_WIDTH-1:0] u8_waddr; 
    (* mark_debug="false" *)wire wen;  
    (* mark_debug="false" *)wire ren; 
       
    axi_slave #( .AXI4_LITE("true"), .ADDR_WIDTH(`FREQ_AXI_ADDR_WIDTH) ) 
      axi_slave_inst ( 
       
        .awaddr  (awaddr), 
        .awvalid (awvalid), 
        .awready (awready), 
        .awlen   (awlen), 
        .awsize  (awsize), 
        .awburst (awburst),
        
        .araddr  (araddr), 
        .arvalid (arvalid), 
        .arready (arready), 
        .arlen   (arlen), 
        .arsize  (arsize), 
        .arburst (arburst),
        
        //.rdata  (rdata), 
        .rresp  (rresp), 
        .rvalid (rvalid), 
        .rready (rready), 
        .rlast  (rlast),  
        
        //.wdata  (wdata), 
        //.wstrb  (wstrb), 
        .wvalid (wvalid), 
        .wready (wready), 
        .wlast  (wlast),
      
        .bresp  (bresp), 
        .bvalid (bvalid), 
        .bready (bready),  
        // 
        .u8_waddr (u8_waddr), // u32(physical address) = (u8>>2)  
        .wen      (wen),      // u32(physical address) = (u8>>2)  
        
        .u8_raddr (u8_raddr), 
        .ren      (ren), //  
        // 
        .clk0(clk0), 
        .resetn(resetn) ) ; 

//-----------------------------------------------------	       
// wdata mem write 
   (* mark_debug="false" *)wire [`NUM_COUNTERS-1:0] freq_irq;        // to cpu              
 
	reg [31:0] max_cycle_count[`NUM_COUNTERS-1:0]; // max_freq_count    (R/W write only from cpu) 
    reg [31:0] max_time_count[`NUM_COUNTERS-1:0];  // max_period_count  (R/W) write only from cpu )
    reg [31:0] max_timeout[`NUM_COUNTERS-1:0];     // sets the timeout error delay 
    reg [31:0] freq_CR[`NUM_COUNTERS-1:0];         // freq_counter Control [7:0], clk_sel[1:0], clkdiv[15:0], trig_polarity, signal_polarity 

	wire [`FREQ_MEM_ADDR_WIDTH-1:0] mem_waddr = u8_waddr[`FREQ_AXI_ADDR_WIDTH-1:2]; // %`FREQ_NUM_U32_REGS; 
    wire [7:0] counter_windex = (mem_waddr>>3); // counters are gropued in sets of 8 u32 groups  (max counters = 256) 
    wire [2:0] wreg_index     = (mem_waddr % 8) ; // a single 
    
    integer i; 
    always @(posedge clk0) begin 
       if (~resetn) 
          for(i=0; i<`NUM_COUNTERS; i=i+1) begin
              freq_CR[i] <= #(`HOLD_TIME)'h01_ff_ff_00;  // default value; 
              max_time_count[i]  <= #(`HOLD_TIME)'hffffffff;
              max_cycle_count[i] <= #(`HOLD_TIME)'h1;
              max_timeout[i]     <= #(`HOLD_TIME)32'hfffffff7; // default to 40 sec timeout delay 
              end
       else if (wen) begin 
          if      (wreg_index == 0) freq_CR[counter_windex]         <= #(`HOLD_TIME)wdata; 
          // else if (wreg_index == 1) freq_SR[counter_windex]      <= wdata; 
          else if (wreg_index == 2) max_cycle_count[counter_windex] <= #(`HOLD_TIME)wdata; 
          else if (wreg_index == 3) max_time_count[counter_windex]  <= #(`HOLD_TIME)wdata; 
          //else if (wreg_index == 4) cycle_count[counter_windex]   <= wdata; 
          //else if (wreg_index == 5) time_count[counter_windex]    <= wdata;               
          else if (wreg_index == 6) max_timeout[counter_windex]     <= #(`HOLD_TIME)wdata; 
          end // if 
       end // always                                
               
//--------------------------------------------------------------------------------------------
// rdata mux 	
genvar w; 
generate 
    for (w=0; w<`NUM_COUNTERS; w=w+1) begin : wire_reg
      wire [31:0] freq_SR; 
      wire [31:0] cycle_count;
      wire [31:0] time_count ;     
      end 
endgenerate 

reg [31:0] freq_SR[`NUM_COUNTERS-1:0]; 
reg [31:0] cycle_count[`NUM_COUNTERS-1:0]; 
reg [31:0] time_count[`NUM_COUNTERS-1:0]; 

genvar r; 
generate 
    for(r=0; r<`NUM_COUNTERS; r=r+1) begin : rmux
         always @(*) freq_SR[r]     = wire_reg[r].freq_SR; 
         always @(*) cycle_count[r] = wire_reg[r].cycle_count; 
         always @(*) time_count[r]  = wire_reg[r].time_count ;  
        end
endgenerate 

    wire [`FREQ_MEM_ADDR_WIDTH-1:0] mem_raddr = u8_raddr[`FREQ_AXI_ADDR_WIDTH-1:2]; // %`FREQ_NUM_U32_REGS; 
   
    wire [7:0] counter_rindex = (mem_raddr >> 3); // counters are gropued in sets of 8 u32 groups  
    wire [2:0] rreg_index     = (mem_raddr % 8) ;
    
    assign rdata = 
        (rreg_index == 0) ? freq_CR[counter_rindex]         : 
        (rreg_index == 1) ? freq_SR[counter_rindex]         :            //wire_reg[counter_rindex].freq_SR; //(freq_SR_wire>>(32*counter_rindex)); 
        (rreg_index == 2) ? max_cycle_count[counter_rindex] :  
        (rreg_index == 3) ? max_time_count[counter_rindex]  : 
        (rreg_index == 4) ? cycle_count[counter_rindex]     : //wire_reg[counter_rindex].freq_count; // (freq_count_wire>>(32*counter_rindex)); 
        (rreg_index == 5) ? time_count[counter_rindex]      : // wire_reg[counter_rindex].freq_preiod; // (freq_period_wire>>(32*counter_rindex));
        (rreg_index == 6) ? max_timeout[counter_rindex]     : // wire_reg[counter_rindex].freq_preiod; // (freq_period_wire>>(32*counter_rindex));     
        32'h0;                   
    
          
  genvar n;
  generate 
     for(n=0; n<`NUM_COUNTERS; n=n+1) begin : freq_counters 
            freq_counter freq_counter_inst ( 
              .din              (all_inputs),          // in [`PIN_COUNT-1:0] 
              .cycle_count      (wire_reg[n].cycle_count), // to processor 
              .time_count       (wire_reg[n].time_count),// to processor 
              .max_cycle_count  (max_cycle_count[n]),  // in [31:0] 
              .max_time_count   (max_time_count[n]), // in [31:0]
              .max_timeout      (max_timeout[n]),     // in [31:0] 
              .freq_CR          (freq_CR[n]),         // in [31:0] 
              .freq_SR          (wire_reg[n].freq_SR), 
              .freq_irq         (freq_irq[n]),          // out
              .resetn           (resetn), 
              .clk0             (clk0)
              );       
    end 
  endgenerate  
                         
  assign irq = (freq_irq!=-'h0);
	       
endmodule 

//----------------------------------------------------------------------------------
/*
module freq_pin_select_mux(clk,  all_inputs, sel, out ) ; 
    input [`PIN_COUNT-1:0] all_inputs ; 
    input [7:0] sel;             
    output  out; 
    input clk ; 
//   (* ASYNC_REG="TRUE" *)reg out;  // do not resource share this reg 
//   (* keep="true" *)reg out; // do not resource share this reg   
//    wire [7:0] sub_set = {3'b100, sel[4:0]}; // top 32 bits implies Extra_GPIOs
//    wire [7:0] sub_set = sel[7:0];
    assign out = (sel<`PIN_COUNT)  ? all_inputs[sel] : 1'b0; 
//    always @(posedge clk)  begin 
//       if (sel<`VECTOR_WIDTH) 
//          out <= all_inputs[sel];
//       else 
//           out <= 1'b0; // disabled 
//       end 
endmodule  
*/
//---------------------------------------------------------------------------------- 
 
module freq_counter ( 
    din, 
    cycle_count, time_count, max_cycle_count, max_time_count, max_timeout,
    freq_CR, freq_SR,
    freq_irq,  
    clk0, 
    resetn); 
    
    input clk0;  // FCLK_CLK0
    input resetn; 
    input [`PIN_COUNT-1:0] din; 
    input [31:0] max_cycle_count ;  // used whien in "measure Period" mode 
    input [31:0] max_time_count ; 
    input [31:0] max_timeout; 
    input [31:0] freq_CR; 
    output [31:0] freq_SR; 
    output [31:0] cycle_count; 
    output [31:0] time_count ; 
    output        freq_irq;
     
(* mark_debug="false" *)wire [31:0] freq_SR; 
(* mark_debug="false" *)wire        freq_irq; 
(* mark_debug="false" *)reg [31:0] cycle_count;  
(* mark_debug="false" *)reg [31:0] time_count ;
(* mark_debug="false" *)reg [31:0] timeout_count; 
 
// assign freq_count  = posedge_count; 
// assign freq_period = period_count ; 
 
//(* mark_debug="false" *)reg [31:0] clk_div_counter; 
// 'h01_ff_ff_04 (div=1, no_trigger, no_sig, period_mode) 
// control register bit assignments     
(* mark_debug="false" *)wire enable      = freq_CR[0]; 
(* mark_debug="false" *)wire irq_en     = freq_CR[1];
//(* mark_debug="false" *)wire period_mode = freq_CR[2]; //  measure period mode (default_on) 
//    wire       mode0       = freq_CR[3]; // average 
//    wire       mode1       = freq_CR[4]; // 
//    wire       mode2       = freq_CR[5]; // 
//    wire       mode3       = freq_CR[6]; //  AXI4
//    wire       mode4       = freq_CR[7]; //     
(* mark_debug="false" *)wire [7:0] sig_sel     = freq_CR[15:8] ; 
(* mark_debug="false" *)wire [7:0] trig_sel    = freq_CR[23:16] ; 
//(* mark_debug="false" *)wire [7:0] clk_div_sel = freq_CR[31:24] ; // clk0 / 2**clk_dev

// shadow registers : protect from parameter changes durring measurement 
(* mark_debug="false" *)reg [7:0] sig_sel_reg;     
(* mark_debug="false" *)reg [7:0] trig_sel_reg;   
(* mark_debug="false" *)reg        irq_en_reg;  // clk0 / 2**clk_dev   
(* mark_debug="false" *)reg [31:0] max_cycle_count_reg; 
(* mark_debug="false" *)reg [31:0] max_time_count_reg; 
    
//  (* mark_debug="false", ASYNC_REG="TRUE" *)reg [1:0]sig;
//  (* mark_debug="false", ASYNC_REG="TRUE" *)reg [1:0]trig;
//  (* mark_debug="false", ASYNC_REG="TRUE" *)reg [1:0]pclk;  

  (* mark_debug="false", ASYNC_REG="TRUE" *)reg [1:0]sig;
  (* mark_debug="false", ASYNC_REG="TRUE" *)reg [1:0]trig;

//(* ASYNC_REG="TRUE" *)reg sig;
//(* ASYNC_REG="TRUE" *)reg trig;
//(* ASYNC_REG="TRUE" *)reg pclk;  

`define FREQ_IDLE  1 
`define FREQ_WAIT  2
`define FREQ_RUN   4 
//`define FREQ_LAST  0
`define FREQ_DONE  8 

(* mark_debug="false" *)wire sig_net ;// = (sig_sel_reg<`PIN_COUNT)  ? din[sig_sel_reg] : 1'b0 ;//din[`PIN_COUNT-1];   
(* mark_debug="false" *)wire trig_net ;//= (trig_sel_reg<`PIN_COUNT) ? din[trig_sel_reg]: 1'b0 ;//din[`PIN_COUNT-2];   
//(* mark_debug="false" *)wire pclk_net = (clk_div_counter[clk_div_sel_reg[4:0]]);

(* mark_debug="false" *) reg [3:0] state; 
   wire [3:0] next_state; 
  // wire [4:0] freq_next_state; 
   //wire [4:0] period_next_state; 
   
   assign freq_irq = (state==`FREQ_DONE) & irq_en_reg;         
   (* mark_debug="false" *)wire done    = (state==`FREQ_DONE); 
   (* mark_debug="false" *)wire idle    = (state==`FREQ_IDLE);     
   (* mark_debug="false" *)wire running = (state==`FREQ_RUN);     
   (* mark_debug="false" *)wire waiting = (state==`FREQ_WAIT);     
   
   (* mark_debug="false" *)reg sig_timeout_err; 
   always @(posedge clk0) 
        if (idle) 
            sig_timeout_err <= 0; 
        else if (waiting & timeout )
            sig_timeout_err <= 1;
             
   //freq_pin_select_mux trig_sel_mux( .clk(clk0), .all_inputs(din), .sel(trig_sel_reg), .out(trig_net) ); 
   //freq_pin_select_mux  sig_sel_mux( .clk(clk0), .all_inputs(din), .sel(sig_sel_reg),  .out(sig_net) );   
    
   assign sig_net  = (sig_sel_reg<`PIN_COUNT)  ? din[sig_sel_reg]  : 1'b0;
   assign trig_net = (trig_sel_reg<`PIN_COUNT) ? din[trig_sel_reg] : 1'b0; //   

   // pos edge detectors (always running to avoid false triggers)  
   always @(posedge clk0  ) begin 
          sig[0]  <= sig_net; 
          trig[0] <= trig_net;
          sig[1] <= sig[0]; 
          trig[1] <= trig[0];
          end
          
   (* mark_debug="false" *)wire sig_posedge  = ~sig[1]  & sig[0];//sig_net  & ~sig; //sig[0]  & ~sig[1]; 
   (* mark_debug="false" *)wire trig_posedge = ~trig[1] & trig[0]; //trig_net & ~trig;// & ~trig[1]; 
//   (* mark_debug="false" *)wire pclk_posedge = ~pclk[1] & pclk[0]; //pclk_net & ~pclk; // & ~pclk[1];              
      
    (* mark_debug="false" *)wire last_period  = (time_count  == max_time_count_reg-1);
    (* mark_debug="false" *)wire last_posedge = (cycle_count == max_cycle_count_reg-1);
    (* mark_debug="false" *)wire timeout      = (timeout_count >= max_timeout) ; // 40 sec timeout (note max_timeout does not have a shadow reg) 
                                                                                // so it can be used to Immediate abort and goto done.     
    assign next_state = 
        (~enable)                                           ? `FREQ_IDLE : // disable aborts counter 
        ((state ==`FREQ_IDLE) & (trig_sel_reg=='hfe ))      ? `FREQ_WAIT : // Immediate  trigger start the counter 
        ((state ==`FREQ_IDLE) & trig_posedge)               ? `FREQ_WAIT : // first posedge start the counter 
        ((state ==`FREQ_IDLE) & timeout )                   ? `FREQ_DONE : // timeout waiting for trig posedge
        ((state ==`FREQ_WAIT) & sig_posedge)                ? `FREQ_RUN  : // first posedge starts counters 
        ((state ==`FREQ_WAIT) & timeout)                    ? `FREQ_DONE : // timeout error waiting for first posedge 
        ((state ==`FREQ_RUN) & last_period )                ? `FREQ_DONE : 
        ((state ==`FREQ_RUN) & last_posedge & sig_posedge)  ? `FREQ_DONE : 
        ((state ==`FREQ_DONE) & ~enable )                   ? `FREQ_IDLE :             
        state; //  
                          
     always @(posedge clk0 or negedge resetn) 
        if (~resetn)
            state <= `FREQ_IDLE; 
        else 
            state <= next_state ; 
                                 
     // status register bit assignments 
     assign freq_SR[0]     = done    ; //(state==`FREQ_DONE); 
     assign freq_SR[1]     = idle    ; //(state==`FREQ_IDLE); 
     assign freq_SR[2]     = waiting ; //(state==`FREQ_WAIT); 
     assign freq_SR[3]     = running ; //(state==`FREQ_RUN); 
     
     assign freq_SR[4]     = irq_en_reg; 
     assign freq_SR[5]     = ((time_count==0) |(cycle_count==0)) & done; // indicates invalid test (parameter problem or timeout 
     assign freq_SR[6]     = sig_timeout_err; // 'b0; //max_error      ; // max_freq == 0 or max_period = 0; 
     assign freq_SR[7]     = timeout  ; // timeout error indicate no trigger or no posedge or max_period  == 0; 
     //assign freq_SR[7:4]   = state[3:0];
     assign freq_SR[31:24] = 'h0; //{3'b000,clk_div_sel_reg}; 
     assign freq_SR[23:16] = trig_sel_reg; 
     assign freq_SR[15:8]  = sig_sel_reg; 
 
    // clock divider counter  (measures time)                    
    always @(posedge clk0 ) 
        if (~resetn)
            time_count <= 0; 
       else if (state== `FREQ_IDLE) 
            time_count <= 0; 
        else if (running)
            time_count <= time_count +1 ; 
     
    // timeout counter         
    wire reset_timeout = ~enable | 
                    ((state==`FREQ_IDLE) & (next_state == `FREQ_WAIT)) |
                    ((state==`FREQ_WAIT) & (next_state == `FREQ_RUN)) ;
                    
    always @(posedge clk0 ) begin 
        if (~resetn)
            timeout_count <= 0; 
        else if  (reset_timeout) 
            timeout_count <= 0; 
        else if (timeout_count == 32'hffffffff) // no roll over 
            timeout_count <= 32'hffffffff; 
        else if  ((state == `FREQ_WAIT) | (state==`FREQ_IDLE)) 
            timeout_count <= timeout_count + 1; 
        end     
        
    // frequency  counter  (counts posedges)    
    always @(posedge clk0  ) 
        if (~resetn) 
            cycle_count  <= 0; 
        else if (state==`FREQ_IDLE) 
            cycle_count <= 0 ; 
        else if ((running) & sig_posedge) 
            cycle_count <= cycle_count + 1; 
            
 // shadow registers (prevent parameter change while running) except for max_timeout 
    always @(posedge clk0  ) begin
        if (~resetn)  begin 
            sig_sel_reg     <= 'hff; // default off 
            trig_sel_reg    <= 'hff; // default off 
            irq_en_reg      <= 0;  
            max_time_count_reg  <= 32'hffffffff; 
            max_cycle_count_reg <= 32'h00000001; 
            end 
        else if (state==`FREQ_IDLE) begin 
            sig_sel_reg     <= sig_sel; 
            trig_sel_reg    <= trig_sel; 
            irq_en_reg      <= irq_en;  
            max_time_count_reg  <= (max_time_count ==0) ? 'h1 : max_time_count; // cant have a 0 number of periods 
            max_cycle_count_reg <= (max_cycle_count==0) ? 'h1 : max_cycle_count;  // cant have 0 posedge count 
            end  
        end 
                      
endmodule
