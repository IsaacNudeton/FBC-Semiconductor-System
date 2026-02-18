
`timescale 1 ns / 1 ps
`include  "vector.vh"

`define  VECSTAT_AXI4_LITE  // don't need burst transfers 
 
`define VECSTAT_AXI_ADDR_WIDTH 14 // 4KByte block of u32 space 
`define VECSTAT_MEM_ADDR_WIDTH  4 // 16 u32 addresses => 4 bits
`define VECSTAT_MEM_DATA_WIDTH 32 
`define VECSTAT_NUM_U32_REGS   16 // 16*4 = 64 bytes 

`include  "axi_slave.vh"

module axi_vector_status ( 
	final_gap_count, final_error_count, final_vector_count, final_cycle_count, 
	errors_detected, 
	done_irq, repeat_count_done,
	awaddr, awvalid, awready, awlen, awsize, awburst,
	araddr, arvalid, arready, arlen, arsize, arburst,
	rdata, rresp, rvalid, rready, rlast, // rid, ruser 
	wdata, wstrb, wvalid, wready, wlast, // wid, wuser 
	bresp, bvalid, bready,  
	clk0, resetn 
  ) ; 
	input clk0 ; 
	input resetn; 

	input  [31:0] final_gap_count;      
  	input  [`ERROR_COUNT_WIDTH:0] final_error_count;    
  	input  [31:0] final_vector_count;    
  	input  [63:0] final_cycle_count;   //
  	input         errors_detected; 
  	input         repeat_count_done; 
	output        done_irq;
	// write address channel 
	input  [`VECSTAT_AXI_ADDR_WIDTH-1:0] awaddr ; 
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
	input  [`VECSTAT_AXI_ADDR_WIDTH-1:0] araddr ; 
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


    wire [`VECSTAT_AXI_ADDR_WIDTH-1:0] u8_waddr; 
    wire [`VECSTAT_AXI_ADDR_WIDTH-1:0] u8_raddr; 
    wire wen;  
    wire ren; 
       
    axi_slave #( .AXI4_LITE("true"), .ADDR_WIDTH(`VECSTAT_AXI_ADDR_WIDTH) ) 
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


   (*mark_debug="true" *)wire [`VECSTAT_MEM_ADDR_WIDTH-1:0] mem_waddr = u8_waddr[`VECSTAT_AXI_ADDR_WIDTH-1:2]; // %`VECSTAT_NUM_U32_REGS; 

   (*mark_debug="true" *)reg  irq_en_reg; // {irq_clear, irq_en, irq_clr, 
   always @(posedge clk0) 
        if (~resetn)  
          irq_en_reg   <= #(`HOLD_TIME)'h0 ;
       else if (wvalid & wready & (mem_waddr==6))
          irq_en_reg  <= #(`HOLD_TIME)wdata[0]; 

//------------ done status behavior ---------------------------
// repeat_count_done goes low one cycle before first compare and goes high after last comare (and stays high there after) 
// this needs a timeout counter to detect catistrophic hang conditions (to insures completion) 
    (*mark_debug="true" *)reg [63:0] timeout; 
    (*mark_debug="true" *)reg [63:0] max_timeout; 
    (*mark_debug="true" *)wire  timeout_event = (timeout>=max_timeout); 
    (*mark_debug="true" *)reg [1:0] done_state;// = repeat_count_done;// read only  
    
    always @(posedge clk0) 
        if (~resetn)  
            max_timeout  <= #(`HOLD_TIME) 64'h0000_0007_ffff_ffff ; // default to 5 minutes 
         else if (wvalid & wready & (mem_waddr==9))
            max_timeout[63:32]  <= #(`HOLD_TIME)wdata; 
         else if (wvalid & wready & (mem_waddr==8)) // lsb first and u64 alignment 
            max_timeout[31:0]  <= #(`HOLD_TIME)wdata;     
                   
//--------------- irq status bit (read / write 0 clear) ----------------------------------------   
 
   `define VEC_RST  2'h3 // IDLE STATE 
   `define VEC_RUN  2'h0 // repeat_count_done negedge has occrued 
   `define VEC_DONE 2'h1 // repeat_count_done posedge has occured 
   `define VEC_ERR  2'h2 // timeout error state 
   
   wire [1:0] next_done_state;
   assign next_done_state = 
        ((done_state == `VEC_RST) & ~repeat_count_done) ? `VEC_RUN : 
        ((done_state == `VEC_RUN) &  repeat_count_done) ? `VEC_DONE : 
        ((done_state == `VEC_RUN) &  timeout_event)     ? `VEC_ERR :         
        ((done_state == `VEC_DONE) & wvalid & wready & (mem_waddr==5) ) ? `VEC_RST :
        ((done_state == `VEC_ERR ) & wvalid & wready & (mem_waddr==5) ) ? `VEC_RST :
        done_state ; 
             
    (* mark_debug="true" *)wire  done_status_reg = (done_state==`VEC_DONE);
    (* mark_debug="true" *)wire  timeout_reg     = (done_state==`VEC_ERR);
     
    always @(posedge clk0) 
        if (~resetn) 
            done_state<=`VEC_RST; 
        else 
            done_state <= next_done_state; 
    
    (* mark_debug="true" *)assign done_irq = irq_en_reg & (done_status_reg | timeout_reg); 
    
    always @(posedge clk0) 
         if (done_state==`VEC_RST)  
            timeout<='h0; 
         else if (done_state==`VEC_RUN) 
            timeout <= timeout+1;
               
//--------------------------------------------------------------------------------------------
// rdata mux 	
   (* mark_debug = "true" *)wire [`VECSTAT_MEM_ADDR_WIDTH-1:0] mem_raddr = u8_raddr[`VECSTAT_AXI_ADDR_WIDTH-1:2]; // %`VECSTAT_NUM_U32_REGS; 

   assign rdata = 
        (mem_raddr == 0) ? final_error_count        : 
        (mem_raddr == 1) ? final_vector_count       : 
        (mem_raddr == 2) ? final_cycle_count[31:0]  : // lsb first and u64 allignment
        (mem_raddr == 3) ? final_cycle_count[63:32] :
        (mem_raddr == 4) ? final_gap_count          : 
        (mem_raddr == 5) ? {30'h0, timeout_reg, done_status_reg} : // irq status (R/WClear)  
        (mem_raddr == 6) ? {31'h0,      irq_en_reg} : //irq_control; 
        (mem_raddr == 7) ? {31'h0, errors_detected} : // (Read ONLY)  
        (mem_raddr == 8) ? max_timeout[31:0]        : // lsb first and u64 alignment
        (mem_raddr == 9) ? max_timeout[63:32]       : 
        (mem_raddr ==10) ? timeout[31:0]            : // read only lsb u64 alligned
        (mem_raddr ==11) ? timeout[63:32]           : // read only msb
        (mem_raddr ==15) ? `FPGA_VERSION            : // design version 
        32'h0;                     
         
endmodule 
