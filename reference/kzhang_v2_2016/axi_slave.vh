`ifndef _AXI_SLAVE_H_
`define _AXI_SLAVE_H_ 

//------------------ AWCACHE VAlues ------------------------
`define CACHE_TYPE_NON_BUFFERABLE   	4'b0000  //ARCACHE  
`define CACHE_TYPE_BUFFERABLE 			4'b0001  
`define CACHE_TYPE_NONCACHE_NOBUF   	4'b0010  
`define CACHE_TYPE_NONCACHE_BUF 		4'b0011  

`define CACHE_TYPE_WRITE_THROUGH_NOALLOC 	4'b0110 // ARCACHE(1010)  
//`define CACHE_TYPE_WRITE_THROUGH_READ_ALLOC 	4'b0110 // ???? 
//`define CACHE_TYPE_WRITE_THROUGH_WITE_ALLOC 	4'b1110 // ???? 
`define CACHE_TYPE_WRITE_THROUGH_READ_WITE_ALLOC 4'b1110  

`define CACHE_TYPE_WRITEBACK_NOALLOC 		4'b0111  
`define CACHE_TYPE_WRITEBACK_READ_ALLOC 	4'b0111  
`define CACHE_TYPE_WRITEBACK_WRITE_ALLOC 	4'b1111 // 1011 ??  
`define CACHE_TYPE_WRITEBACK_READ_WRITE_ALLOCC 	4'b111  

//`define ID_WIDTH 2 
//`define USER_WIDTH 2 
//`define REGION_WIDTH 2 

`define RESP_OK      2'b00
`define RESP_EXOK    2'b01
`define RESP_SLVERR  2'b10
`define RESP_DECERR  2'b11

//-----------------------------------------------------------------------------
// OKAY response indicates any one of the following:
// 	1: the success of a normal access
// 	2: the failure of an exclusive(EX) access
//	3: an ex access to a slave that does not support exclusive access.
// 	OKAY is the response for most transactions.
// 	
// EXOKAY response indicates the success of an exclusive access. 
//
// SLVERR response indicates an unsuccessful transaction.
//    To simplify system monitoring and debugging, it is recommended that 
//    error responses are used only for error conditions and not for signaling 
//    normal, expected events. 
//    Examples of slave error conditions are:
//	1: FIFO or buffer overrun or underrun condition
//	2: unsupported transfer size attempted
//	3: write access attempted to read-only location
//	4: timeout condition in the slave
//	5: access attempted to a disabled or powered-down function.
//
// DECERR response indicates the interconnect cannot successfully decode a slave access.
//    If interconnect cannot successfully decode a slave access, it must return the DECERR. 
//    This specification recommends that the interconnect routes the access to 
//    a default slave, and the default slave returns the DECERR response.
//    The AXI protocol requires that all data transfers for a transaction are completed, 
//    even if an error condition occurs.
//    Any component giving a DECERR response must meet this requirement.
//------------------------------------------------------------------------------

//NOTE: bytes_per_wvalid = (1<<awsize[2:0])

//burst types 
`define BURST_FIXED 2'b00 // next_addr = addr; 
`define BURST_INC   2'b01 // next_addr = addr+bytes_per_wvalid
`define BURST_WRAP  2'b10 // address wraps around to a lower address if an upper address limit is reached.
`define BURST_RESRV 2'b11 // reseved 

//------------------------------------------------------------------------------
// Wrap byrst type : 
// The following restrictions apply to wrapping bursts:
// 	1: the start address must be aligned to the size of each transfer
// 	2: the length(awlen) of the burst must be 2, 4, 8, or 16 transfers.
// The behavior of a wrapping burst is:
// 	1: The lowest address used by the burst is aligned to the total size 
// 	   (awsize*awlen) of the data to be transferred
//         that is, to 
//         ((size of each transfer in the burst)�(number of transfers in the burst)). 
//         This address is defined as the wrap boundary
// 	2: After each transfer, the address increments in the same way as for 
// 	   an INCR burst. However, if this incremented address is 
// 	   ((wrap boundary) + (total size of data to be transferred)) 
// 	   then the address wraps round to the wrap boundary
// 	3: The first transfer in the burst can use an address that is higher 
// 	   than the wrap boundary, subject to the restrictions that apply to 
// 	   wrapping bursts. This means that the address wraps for any WRAP burst
// 	   for which the first address is higher than the wrap boundary. 
// 	   This burst type is used for cache line accesses.
//------------------------------------------------------------------------------

`define AXI_WR_IDLE   'h0  // awready   
`define AXI_WR_DATA   'h1  // wready
`define AXI_WR_BRSP   'h2  // bvalid 

`define AXI_RD_IDLE   'h0 // arready,   
`define AXI_RD_DATA   'h1 // rvalid & rlast  

`define PAGE_BLOCK_SIZE 4096 // 4096 byte page boundries  

`endif 
