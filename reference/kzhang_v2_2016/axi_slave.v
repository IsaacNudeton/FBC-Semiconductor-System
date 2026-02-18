`timescale 1 ns / 1 ps

`include "axi_slave.vh" 

module axi_slave   #( 
    parameter AXI4_LITE  = 1,    // Specify AXI4_LITE or not (defaults to true)  
    parameter ADDR_WIDTH = 14    // Specify (number of address bits )
  ) (  
    awaddr, awvalid, awready, awlen, awsize, awburst,
    araddr, arvalid, arready, arlen, arsize, arburst,
    //rdata, 
    rresp, rvalid, rready, rlast, // rid, ruser 
    //wdata, wstrb, 
    wvalid, wready, wlast, // wid, wuser 
    bresp, bvalid, bready,  
    // 
    u8_waddr, wen, // u32(physical address) = (u8>>2)  
    u8_raddr, ren, //  

    // 
    clk0, resetn ) ; 

    input clk0 ; 
    input resetn; 

    output [ADDR_WIDTH-1:0] u8_waddr;  
    output [ADDR_WIDTH-1:0] u8_raddr;  
    output   wen;           // = (wvalid & wready) 
    output   ren;           // = (rvalid & rready)  

    // write address channel 
    input [ADDR_WIDTH-1:0] awaddr ; 
    input  [7:0] awlen ; 
    input  [2:0] awsize ; 
    input  [1:0] awburst ; 
    input        awvalid; 
    output       awready ; 
//`ifndef AXI_PROTECTION
//    input [1:0] awlock;  // lock Type [???} 
//    input [3:0] awcache; // Memory Access Type identiver  
//    input [2:0] awprot;  // {Ins(1)/Data(0)[0], non-secure[2], Priviledge_access[0],,   
//    input [3:0] awqos; // quality of service transation attribute ???  
//    input [`REGION_WIDTH-1:0] awregion; // memory Regionattribute  
//    input [`ID_WIDTH-1:0] awid; // waddr ID (transaciotn order)   
//    input [`USER_WIDTH-1:0] awuser;   //  User Defined Extra 
// `endif 

    // read Address channel     
    input [ADDR_WIDTH-1:0] araddr ; 
    input        arvalid; 
    output       arready ; 
    input  [7:0] arlen;   // num of transfers per burst (per awaddr)  
    input  [2:0] arsize;  // buret bytes size {1,2,4, 8, 16, 32, 64, 128}   
    input  [1:0] arburst; // burst type {FIXED, INCR, WRAP, RESERVED } 
// `ifdef AXI_SECURITY  
//    input [1:0] arlock;  // lock Type [???} 
//    input [3:0] arcache; // Memory Access Type identiver  
//    input [2:0] arprot;  // {Ins(1)/Data(0)[0], non-secure[2], Priviledge_access[0],,   
//    input [3:0] arqos; // quality of service transation attribute ???  
//    input [`REGION_WIDTH-1:0] arregion; // memory Region attribute  
// `endif 
// `ifdef AXI_TRANS_ORDERING // interconnects  
//    input [`ID_WIDTH-1:0]     arid;    // waddr ID (transaciotn oreder)   
//    input [`USER_WIDTH-1:0]   aruser;  // waddr User control  
//`endif 

    // read data channel (to master ) 
    //output [31:0] rdata ; 
    output  [1:0] rresp ; 
    output        rvalid; 
    input         rready ; 
    output        rlast ; 
    //output  [`ID_WIDTH-1:0]  rid; 
    //output  [`USER_WIDTH-1:0]  ruser; 

    // write data channel (from master ) 
    //input  [31:0] wdata ; 
    //input   [3:0] wstrb ; // byte mask 
    input         wvalid; 
    output        wready ; 
    input         wlast ;  // end od burst 
    //output  [`ID_WIDTH-1:0]  wid; 
    //output  [`USER_WIDTH-1:0]  wuser; 

    // write response channel 
    output [1:0] bresp; 
    output       bvalid; 
    input        bready; 
//    output [`ID_WIDTH-1n:0] bid  ;  // ID tag
//    output [`USER_WIDTH-1n:0] buser;  // ID tag

    (* mark_debug="false" *)reg [7:0] bytes_per_wvalid; // = (1<<awsize[2:0]); 
    (* mark_debug="false" *)reg [7:0] waddr_count ;     // = [awlen..0] 
    (* mark_debug="false" *)reg [2:0] awburst_reg;
     
    (* mark_debug="false" *)reg [7:0] bytes_per_rvalid; // = (1<<arsize[2:0]);  
    (* mark_debug="false" *)reg [7:0] raddr_count ;     // = [arlen..0]  
    (* mark_debug="false" *)reg [2:0] arburst_reg;

    (* mark_debug="false" *)wire [31:0] rdata;  
    (* mark_debug="false" *)reg   [1:0] write_state ; 
    (* mark_debug="false" *)reg         read_state ; 
    (* mark_debug="false" *)reg  [ADDR_WIDTH-1:0] waddr_reg ;  // max block = 4096 bytes
    (* mark_debug="false" *)reg  [ADDR_WIDTH-1:0] raddr_reg ;  // max block = 9
    
    wire wready;  // 1 after awaddr, 0 after wvalid   
    wire awready; // 0 after wvalid  
    wire bvalid ; 
    wire rvalid; 
    wire rlast; 

    wire wslverr = ( 
           (awburst_reg == `BURST_WRAP) | (awburst_reg == `BURST_RESRV) | // unsupported wburst types                 
//          (bytes_per_wvalid > 4)                                      | // size errors
           ((waddr_count==0) & (wlast==0) & (wvalid&wready))            | // premature wlast detected  
           (waddr_reg >= `PAGE_BLOCK_SIZE)                                // page boundry alignment violation 
           ) ;
                     
    assign bresp = (wslverr)?`RESP_SLVERR : `RESP_OK; 
     
    assign awready = (write_state==`AXI_WR_IDLE) ; 
    assign wready  = (write_state==`AXI_WR_DATA) ;                     
    assign bvalid  = (write_state==`AXI_WR_BRSP) ;  

    wire last_w = (AXI4_LITE=="true") ? 1'b1 : wlast ; 

    wire [1:0] next_write_state =
           (write_state == `AXI_WR_IDLE ) & (awvalid)         ? `AXI_WR_DATA :
           (write_state == `AXI_WR_DATA ) & (wvalid & last_w) ? `AXI_WR_BRSP :
           (write_state == `AXI_WR_DATA ) & (wvalid)          ? `AXI_WR_DATA :     
           (write_state == `AXI_WR_BRSP ) & (bready)          ? `AXI_WR_IDLE :     
           write_state; 

    always @(posedge clk0 or negedge resetn) 
       if (~resetn) begin 
           waddr_reg        <= 'h0 ;
           awburst_reg      <= `BURST_INC; // default burst type 
           waddr_count      <= 'h0; 
           bytes_per_wvalid <= 'h4; // default to 1 byte mode 
           end  
       else if (awvalid & awready) begin 
           waddr_reg   <= awaddr; 
           waddr_count <= (AXI4_LITE=="true") ? 'h0 : awlen  ; 
           awburst_reg <= (AXI4_LITE=="true") ? `BURST_INC : awburst; 
           bytes_per_wvalid <= (awburst==`BURST_INC) ? (1<<awsize) : 'h4 ; 
           end  
       else if (wvalid & wready &(AXI4_LITE=="true")) begin 
           waddr_reg   <= waddr_reg + bytes_per_wvalid ; 
           waddr_count <= (waddr_count>0)?(waddr_count - 8'h1):8'h0; 
           end             

    always @(posedge clk0 or negedge resetn)  
        if (~resetn)   
          write_state <= `AXI_WR_IDLE; 
        else   
          write_state <= next_write_state;  

//---------- read state machine -------------------------------------------           
   wire rslverr = ( 
         (arburst_reg == `BURST_WRAP) | (arburst_reg == `BURST_RESRV) |// unsupported rburst types   
         (raddr_reg >= `PAGE_BLOCK_SIZE) // page boundry alignment violation 
      ) ;
                  
    wire  next_read_state = 
        (read_state == `AXI_RD_IDLE ) & (arvalid)          ? `AXI_RD_DATA :     
        (read_state == `AXI_RD_DATA ) & (rready & rlast)   ? `AXI_RD_IDLE :     
        (read_state == `AXI_RD_DATA ) & (rready)           ? `AXI_RD_DATA :     
        read_state; 
    
    assign arready = (read_state == `AXI_RD_IDLE); 
    assign rvalid  = (read_state == `AXI_RD_DATA); 
    assign rlast   = (read_state == `AXI_RD_DATA)&(raddr_count==0) ; 

    assign rresp   = (rslverr) ? `RESP_SLVERR : `RESP_OK ; 
 
    assign ren = (rvalid & rready) ;  
        
    always @(posedge clk0 or negedge resetn )  
        if (~resetn)   
            read_state <= `AXI_RD_IDLE; 
        else   
            read_state <= next_read_state;  
            
    always @(posedge clk0 ) 
        if (~resetn) begin 
           raddr_reg        <= 'h0 ;
           raddr_count      <= 'h0 ;  
           arburst_reg      <= `BURST_INC; // default burst type 
           bytes_per_rvalid <= 'h4 ; // default to 32 bit mode 
           end  
        else if (arvalid & arready) begin 
           raddr_reg        <= araddr; 
           raddr_count      <= (AXI4_LITE=="true") ? 'h0        : arlen;   
           arburst_reg      <= (AXI4_LITE=="true") ? `BURST_INC : arburst;
           bytes_per_rvalid <= (AXI4_LITE=="true") ? 'h4        : (1<<arsize) ; 
           end 
        else if (rvalid & rready & (AXI4_LITE!="true") ) begin 
           raddr_reg   <= raddr_reg + bytes_per_rvalid;  
           raddr_count <= (raddr_count>0)?(raddr_count - 8'h1) : 8'h0 ;     
           end  
           
//-------------------------------------------------------------------------------------------           
    assign ren = (rready & rvalid)  ;  
    assign wen = (wready & wvalid)  ;  
    assign u8_waddr = waddr_reg[ADDR_WIDTH-1:0]; // u32 physical addr = (u8 physical addr >> 2)  
    assign u8_raddr = raddr_reg[ADDR_WIDTH-1:0]; // u32 physical addr = (u8 physical addr >> 2)  
//--------------------------------------------------------------------------------------------        
           
endmodule 
