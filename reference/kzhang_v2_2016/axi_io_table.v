`timescale 1 ns / 1 ps

`define IO_TABLE_AXI4_LITE // don't include BURST capabilities 

`define IO_TABLE_ADDR_WIDTH     14 /// 4KByte block of u32 space 
`define IO_TABLE_MEM_ADDR_WIDTH  5 // 32 u32 addresses => 5 bit mem; 
`define IO_TABLE_MEM_DATA_WIDTH 32 

`define IO_TABLE_NUM_U32_REGS  32 //(1<<`IO_TABLE_MEM_ADDR_WIDTH) //(`CONTROL_BIT_WIDTH/32)  // 32*16 = 512 bits
`define CONTROL_BIT_WIDTH     512 //(`IO_TABLE_MEM_DATA_WIDTH*`IO_TABLE_NUM_U32_REGS) // must be u32 aligned (n*32); n==1..1024 

`include "axi_slave.vh" // generic AXI_SLAVE interface definitions   

module axi_io_table( 
	control_bits, 
	delay0,delay1, // u32 values used to delay vec_clk_180 and vec_clk_90
	awaddr, awvalid, awready, awlen, awsize, awburst,
	araddr, arvalid, arready, arlen, arsize, arburst,
	rdata, rresp, rvalid, rready, rlast, // rid, ruser 
	wdata, wstrb, wvalid, wready, wlast, // wid, wuser 
	bresp, bvalid, bready,  
	clk0, resetn 
  ) ; 
	input clk0 ; 
	input resetn; 
	output [`CONTROL_BIT_WIDTH-1:0] control_bits; 
	output [31:0] delay0; 
	output [31:0] delay1; 

	// write address channel 
	input  [`IO_TABLE_ADDR_WIDTH-1:0] awaddr ; 
	input  [7:0] awlen ; 
	input  [2:0] awsize ; 
	input  [1:0] awburst ; 
	input        awvalid; 
	output       awready ; 
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
	input  [`IO_TABLE_ADDR_WIDTH-1:0] araddr ; 
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

    wire [`IO_TABLE_ADDR_WIDTH-1:0] u8_waddr; 
    wire wen; 
    wire [`IO_TABLE_ADDR_WIDTH-1:0] u8_raddr;
    wire ren; 
    
    reg [`CONTROL_BIT_WIDTH-1:0] control_bits; 
 (* MARK_DEBUG="false" *)   reg [31:0] delay0; 
 (* MARK_DEBUG="false" *)   reg [31:0] delay1; 

    axi_slave #( .AXI4_LITE("true"), .ADDR_WIDTH(`IO_TABLE_ADDR_WIDTH) ) 
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

//-------------------------------------------------------------------------------------------	       

(* MARK_DEBUG="false" *) wire [`IO_TABLE_ADDR_WIDTH-1:0] mem_waddr = u8_waddr[`IO_TABLE_ADDR_WIDTH-1:2]; // %`IO_TABLE_NUM_U32_REGS; 
(* MARK_DEBUG="false" *) wire [`IO_TABLE_ADDR_WIDTH-1:0] mem_raddr = u8_raddr[`IO_TABLE_ADDR_WIDTH-1:2]; // % `IO_TABLE_NUM_U32_REGS; 
    
//--------------------------------------------------------------------------------------------		
// rdata mux 

	assign rdata = (mem_raddr<16)  ? (control_bits >> (32*mem_raddr)) : 
	               (mem_raddr==16) ? delay0 : 
	               (mem_raddr==17) ? delay1 :  	  
                   'h0 ; // default 
                   
    wire [`CONTROL_BIT_WIDTH-1:0] shifted_data = (wdata<<(32*mem_waddr)); 
    wire [`CONTROL_BIT_WIDTH-1:0] or_mask = (32'hffffffff<<(32*mem_waddr));  // (wdata<<mem_waddr); 
    
    always @(posedge clk0)  
        if (~resetn)  
            control_bits <=0; 
        else if (wen&&(mem_waddr<16))  
	        control_bits <= (control_bits & ~or_mask) | (shifted_data & or_mask) ; 
	       
	 always @(posedge clk0)  
         if (~resetn)  
             delay0 <=128; 
         else if (wen&&(mem_waddr==16))  
             delay0  <= wdata[7:0]; 
            
	 always @(posedge clk0)  
         if (~resetn)  
             delay1 <=64; 
         else if (wen&&(mem_waddr==17))  
             delay1  <= wdata[7:0];                   
	       
endmodule 
