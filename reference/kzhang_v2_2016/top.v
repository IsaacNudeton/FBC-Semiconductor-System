//------------------------------------------------------------------------------
`timescale 1 ns / 1 ps
`include "vector.vh" 
//`include "./axi_slave.v"

module top
  (DDR_addr,
   DDR_ba,
   DDR_cas_n,
   DDR_ck_n,
   DDR_ck_p,
   DDR_cke,
   DDR_cs_n,
   DDR_dm,
   DDR_dq,
   DDR_dqs_n,
   DDR_dqs_p,
   DDR_odt,
   DDR_ras_n,
   DDR_reset_n,
   DDR_we_n,
   FIXED_IO_ddr_vrn,
   FIXED_IO_ddr_vrp,
   FIXED_IO_mio,
   FIXED_IO_ps_clk,
   FIXED_IO_ps_porb,
   FIXED_IO_ps_srstb,
  
   Vp_Vn_v_n, Vp_Vn_v_p,    // temp sensor ADC (fixed) 
   Vaux_v_n,  Vaux_v_p,     // Analog sensor ADC[15:0]
  
   // pwm_PUDC, // output waveform (gpio[157])
  
   clk_out_n, 
   clk_out_p,

   gpio );

   inout wire [14:0] DDR_addr;
   inout wire [2:0]  DDR_ba;
   inout wire        DDR_cas_n;
   inout wire        DDR_ck_n;
   inout wire        DDR_ck_p;
   inout wire        DDR_cke;
   inout wire        DDR_cs_n;
   inout wire [3:0]  DDR_dm;
   inout wire [31:0] DDR_dq;
   inout wire [3:0]  DDR_dqs_n;
   inout wire [3:0]  DDR_dqs_p;
   inout wire        DDR_odt;
   inout wire        DDR_ras_n;
   inout wire        DDR_reset_n;
   inout wire        DDR_we_n;
   
   inout wire        FIXED_IO_ddr_vrn;
   inout wire        FIXED_IO_ddr_vrp;
   inout wire [53:0] FIXED_IO_mio;
   inout wire        FIXED_IO_ps_clk;
   inout wire        FIXED_IO_ps_porb;
   inout wire        FIXED_IO_ps_srstb;

   input wire        Vp_Vn_v_n;
   input wire        Vp_Vn_v_p; 
   input wire [15:0] Vaux_v_n;
   input wire [15:0] Vaux_v_p;
   
   output wire [3:0] clk_out_p ; 
   output wire [3:0] clk_out_n ; 

   //  output wire        pwm_PUDC; // gpio[157]
                     inout  wire [`PIN_COUNT-1:0] gpio;

   wire                                           FCLK_CLK0; // fpga AXI_intreface clock (100Mhz) 
   wire                                           FCLK_CLK1; // 200 Mhz clk used for Phase delay vector stream clk (max = fclk0/2) 
   //(* keep="false" *)wire FCLK_CLK2; 
   //(* keep="false" *)wire FCLK_CLK3;

   wire                                           delay_clk;
   wire                                           delay_clk_locked;
   
   //wire [31:0] vec_clk_CR; 
   (* mark_debug="false" *) wire vec_clk;   // delayed by 0 degrees (repeat_counter and input strobe)
   (* mark_debug="false" *) wire vec_clk_180;   // inverted fclk1_0 (180 degree phase) gated with vec_clk_en
   (* mark_debug="false" *) wire vec_clk_90;   // delayed by 90 degrees  gated with vec_clk_en (used to careate Pulse type IO) 
   (* mark_debug="false" *) wire pll_clk0; 
   (* mark_debug="false" *) wire pll_clk1; 
   (* mark_debug="false" *) wire pll_clk2; 
   (* mark_debug="false" *) wire pll_clk3; 

   //(* mark_debug="false" *) wire pclk_en; 
   (* mark_debug="false" *) wire pll_en0; 
   (* mark_debug="false" *) wire pll_en1; 
   (* mark_debug="false" *) wire pll_en2; 
   (* mark_debug="false" *) wire pll_en3; 
   (* mark_debug="false" *) wire fclk1_locked;
   (* mark_debug="true" *) wire vec_clk_en ;
   
   //assign vec_clk_p = vec_clk; //(delay_vec_clk)? vec_clk_delayed: vec_clk; //vec_clk_en_negedge & FCLK_CLK1 ; 
   //assign vec_clk_n = ~vec_clk; 
   /*
    assign clk_out_p[0] = pll_clk0;//&vec_clk_en; // (not used)  
    assign clk_out_p[1] = pll_clk1;//&vec_clk_en; // 100 Mhz CML (Fixed 100Mhz)) 
    assign clk_out_p[2] = pll_clk2;//&vec_clk_en; // 10-25 Mhx single ended 
    assign clk_out_p[3] = pll_clk3;//&vec_clk_en; // 10 Mhz Differential ended 
    assign clk_out_n = ~clk_out_p; 

    OBUFDS vec_clk_buf(
    .O (vec_clk_p),     // Diff_p output (connect directly to top-level port)
    .OB(vec_clk_n),     // Diff_n output (connect directly to top-level port)
    .I (vec_clk));      // Buffer input 
    */ 
   OBUFDS #(.IOSTANDARD("LVDS_25")) clk_out0_buf(
                                                 .O (clk_out_p[0]),     // Diff_p output (connect directly to top-level port)
                                                 .OB(clk_out_n[0]),     // Diff_n output (connect directly to top-level port)
                                                 .I (pll_clk0));      // Buffer input 

   OBUFDS #(.IOSTANDARD("LVDS_25")) clk_out1_buf(
                                                 .O (clk_out_p[1]),     // Diff_p output (connect directly to top-level port)
                                                 .OB(clk_out_n[1]),     // Diff_n output (connect directly to top-level port)
                                                 .I (pll_clk1));      // Buffer input
   
   OBUFDS #(.IOSTANDARD("LVDS_25")) clk_out2_buf(
                                                 .O (clk_out_p[2]),     // Diff_p output (connect directly to top-level port)
                                                 .OB(clk_out_n[2]),     // Diff_n output (connect directly to top-level port)
                                                 .I (pll_clk2));      // Buffer input
   
   OBUFDS #(.IOSTANDARD("LVDS_25")) clk_out3_buf(
                                                 .O (clk_out_p[3]),     // Diff_p output (connect directly to top-level port)
                                                 .OB(clk_out_n[3]),     // Diff_n output (connect directly to top-level port)
                                                 .I (pll_clk3));      // Buffer input              


   (* mark_debug="false" *) wire FCLK_RESET0_N;
   //(* mark_debug="false" *) wire FCLK_RESET1_N;
   //(* mark_debug="false" *) wire FCLK_RESET2_N;
   //(* mark_debug="false" *) wire FCLK_RESET3_N;
   (* mark_debug="false" *) wire FCLK0_peripheral_aresetn;
   //(* mark_debug="false" *) wire FCLK1_peripheral_aresetn;
   //(* mark_debug="false" *) wire dma_reset_out_n;
   wire                                           vec_clk_aresetn; 
   //assign FCLK1_peripheral_aresetn = FCLK0_peripheral_aresetn; // 
   //-------------------------------------------------------------------------------------     
   
   //(* mark_debug="false" *) wire [`M_STREAM_WIDTH-1:0] M_AXIS_tdata;  // from 
   (* mark_debug="false" *) wire [3*`VECTOR_WIDTH-1:0] M_AXIS_tdata;  // from 
   (* mark_debug="true" *) wire                    M_AXIS_tlast;  // from 
   (* mark_debug="true" *) wire                    M_AXIS_tready; //to
   (* mark_debug="true" *) wire                    M_AXIS_tvalid; // from
   
   //(* mark_debug="false" *) wire [32+`VECTOR_WIDTH-1:0] S_AXIS_tdata;  // to dma
   (* mark_debug="false" *) wire                  S_AXIS_tlast;  // to_dma 
   //(* mark_debug="false" *) wire                  S_AXIS_tready; // from dma 
   (* mark_debug="false" *) wire                  S_AXIS_tvalid; // to dma 

   //------------------------------------------------------------------------------------------  
   (* mark_debug="false" *) wire [31:0] final_gap_count;      
   (* mark_debug="false" *)  wire [`ERROR_COUNT_WIDTH:0] final_error_count;    
   (* mark_debug="false" *) wire [31:0] final_vector_count;    
   (* mark_debug="false" *) wire [63:0] final_cycle_count;   //

   (* mark_debug="true" *) wire [63:0] cycle_count;     //    
   (* mark_debug="true" *) wire [31:0] vector_count;    // 
   (* mark_debug="false" *) wire [`ERROR_COUNT_WIDTH:0] error_count;      // 
   (* mark_debug="true" *) wire        first_error_detected;  // 
   //-------------------------------------------------------------------------------------
   
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  next_oen; 
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  next_dout;  
   (* mark_debug="false" *) wire [`REPEAT_WIDTH-1:0]  next_repeat_count; 

   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  dout; 
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  oen ; 
   // (* mark_debug="false" *)  reg [`VECTOR_WIDTH-1:0]  din; 
   (* mark_debug="false" *)  wire [`VECTOR_WIDTH-1:0]  error; 
   // (* mark_debug="false" *) wire [`REPEAT_WIDTH-1:0]  loop_count; 
   
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  pin_dout; 
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  pin_oen ; 
   (* mark_debug="false" *) wire [`VECTOR_WIDTH-1:0]  pin_din; 
   
   wire [31:0]                                    extra_gpio_i;
   wire [31:0]                                    extra_gpio_o;
   wire [31:0]                                    extra_gpio_t; 

`ifdef USE_TTC    
   wire [7:0]                                     ttc0_clk1_sel; 
   wire [7:0]                                     ttc0_clk2_sel; 
   wire [7:0]                                     ttc1_clk1_sel; 
   wire [7:0]                                     ttc1_clk2_sel;  
   
   wire [5:0]                                     TTC_OUT; 
   
   wire                                           TTC0_CLK1_IN; 
   wire                                           TTC0_CLK2_IN ; 
   wire                                           TTC1_CLK1_IN ; 
   wire                                           TTC1_CLK2_IN ; 
`endif    
   
   //-------------------------------------------------------------------------------------
   
   (* mark_debug="false" *) wire [511:0] pin_type; 
   (* mark_debug="false" *) wire [2047:0] pulse_ctrl_bits;
   
   (* mark_debug="false" *)    wire [31:0] io_table_AXI_awaddr;  
   (* mark_debug="false" *)    wire        io_table_AXI_awvalid; 
   (* mark_debug="false" *)    wire        io_table_AXI_awready;
   
   (* mark_debug="false" *)    wire [31:0] io_table_AXI_araddr;
   (* mark_debug="false" *)    wire        io_table_AXI_arvalid;
   (* mark_debug="false" *)    wire        io_table_AXI_arready;

   (* mark_debug="false" *)    wire [31:0] io_table_AXI_rdata; 
   (* mark_debug="false" *)    wire  [1:0] io_table_AXI_rresp; 
   (* mark_debug="false" *)    wire        io_table_AXI_rvalid; 
   (* mark_debug="false" *)    wire        io_table_AXI_rready; 
   (* mark_debug="false" *)    wire        io_table_AXI_rlast; // rid, ruser 

   (* mark_debug="false" *)   wire [31:0] io_table_AXI_wdata; 
   (* mark_debug="false" *)   wire  [3:0] io_table_AXI_wstrb; 
   (* mark_debug="false" *)    wire        io_table_AXI_wvalid; 
   (* mark_debug="false" *)    wire        io_table_AXI_wready; 
   //(* mark_debug="false" *)    wire        io_table_AXI_wlast; // wid, wuser 

   (* mark_debug="false" *)   wire [1:0] io_table_AXI_bresp; // to master 
   (* mark_debug="false" *)    wire       io_table_AXI_bvalid;    // to master
   (* mark_debug="false" *)    wire       io_table_AXI_bready;    // from master
   
   //-----------------------------------------------------------
   
   (* mark_debug="false" *)    wire [31:0] freq_AXI_awaddr;  
   (* mark_debug="false" *)    wire        freq_AXI_awvalid; 
   (* mark_debug="false" *)    wire        freq_AXI_awready;
   
   (* mark_debug="false" *)    wire [31:0] freq_AXI_araddr;
   (* mark_debug="false" *)    wire        freq_AXI_arvalid;
   (* mark_debug="false" *)    wire        freq_AXI_arready;

   (* mark_debug="false" *)    wire [31:0] freq_AXI_rdata; 
   (* mark_debug="false" *)   wire  [1:0] freq_AXI_rresp; 
   (* mark_debug="false" *)    wire        freq_AXI_rvalid; 
   (* mark_debug="false" *)    wire        freq_AXI_rready; 
   (* mark_debug="false" *)   wire        freq_AXI_rlast; // rid, ruser 

   (* mark_debug="false" *)    wire [31:0] freq_AXI_wdata; 
   (* mark_debug="false" *)   wire  [3:0] freq_AXI_wstrb; 
   (* mark_debug="false" *)    wire        freq_AXI_wvalid; 
   (* mark_debug="false" *)    wire        freq_AXI_wready; 
   //(* mark_debug="false" *)    wire        freq_AXI_wlast; // wid, wuser 

   (* mark_debug="false" *)   wire [1:0] freq_AXI_bresp; // to master 
   (* mark_debug="false" *)    wire       freq_AXI_bvalid;    // to master
   (* mark_debug="false" *)    wire       freq_AXI_bready;    // from master
   
   //-----------------------------------------------------------
   
   (* mark_debug="true" *)    wire [31:0] vec_status_AXI_awaddr;  
   (* mark_debug="true" *)    wire        vec_status_AXI_awvalid; 
   (* mark_debug="true" *)    wire        vec_status_AXI_awready;
   
   (* mark_debug="true" *)    wire [31:0] vec_status_AXI_araddr;
   (* mark_debug="true" *)    wire        vec_status_AXI_arvalid;
   (* mark_debug="true" *)    wire        vec_status_AXI_arready;

   (* mark_debug="true" *)    wire [31:0] vec_status_AXI_rdata; 
   (* mark_debug="true" *)    wire  [1:0] vec_status_AXI_rresp; 
   (* mark_debug="true" *)    wire        vec_status_AXI_rvalid; 
   (* mark_debug="true" *)    wire        vec_status_AXI_rready; 
   (* mark_debug="true" *)    wire        vec_status_AXI_rlast; // rid, ruser 

   (* mark_debug="true" *)    wire [31:0] vec_status_AXI_wdata; 
   (* mark_debug="true" *)    wire  [3:0] vec_status_AXI_wstrb; 
   (* mark_debug="true" *)    wire        vec_status_AXI_wvalid; 
   (* mark_debug="true" *)    wire        vec_status_AXI_wready; 
   //(* mark_debug="false" *)    wire        vec_status_AXI_wlast; // wid, wuser 

   (* mark_debug="truee" *)    wire [1:0] vec_status_AXI_bresp; // to master 
   (* mark_debug="true" *)    wire       vec_status_AXI_bvalid;    // to master
   (* mark_debug="true" *)    wire       vec_status_AXI_bready;    // from master

   //-----------------------------------------------------------
   
   (* mark_debug="true" *)    wire [31:0] pulse_ctrl_AXI_awaddr;  
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_awvalid; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_awready;
   
   (* mark_debug="true" *)    wire [31:0] pulse_ctrl_AXI_araddr;
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_arvalid;
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_arready;

   (* mark_debug="true" *)    wire [31:0] pulse_ctrl_AXI_rdata; 
   (* mark_debug="true" *)    wire  [1:0] pulse_ctrl_AXI_rresp; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_rvalid; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_rready; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_rlast; // rid, ruser 

   (* mark_debug="true" *)    wire [31:0] pulse_ctrl_AXI_wdata; 
   (* mark_debug="true" *)    wire  [3:0] pulse_ctrl_AXI_wstrb; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_wvalid; 
   (* mark_debug="true" *)    wire        pulse_ctrl_AXI_wready; 
   //(* mark_debug="false" *)    wire        pulse_ctrl_AXI_wlast; // wid, wuser 

   (* mark_debug="truee" *)    wire [1:0] pulse_ctrl_AXI_bresp; // to master 
   (* mark_debug="true" *)    wire       pulse_ctrl_AXI_bvalid;    // to master
   (* mark_debug="true" *)    wire       pulse_ctrl_AXI_bready;    // from master
   
   //-------------------------------------------------------------------------------------
   
   (* mark_debug="false" *)wire [31:0]              error_PORTB_addr; // input[31:0] to BRA
   (* mark_debug="false" *)wire [`VECTOR_WIDTH-1:0] error_PORTB_din;  // input[127:0] to BRAM 
   //wire [`VECTOR_WIDTH-1:0] error_PORTB_dout; // outut[127:0] from BRAM 
   (* mark_debug="false" *)wire                     error_PORTB_en;   // input to  BRAM 
   (* mark_debug="false" *)wire [15:0]              error_PORTB_we;   // input[15:0] to BRAM 
   //wire         error_PORTB_clk;  // input to BRAM 
   //wire          error_PORTB_rst;  // input to BRAM 
   
   (* mark_debug="false" *)wire [31:0] error_cycle_PORTB_addr; // input to BRAM
   (* mark_debug="false" *)wire [63:0] error_cycle_PORTB_din;  // input to BRAM
   //wire [63:0] error_cycle_PORTB_dout; // outut from BRAM 
   wire                                           error_cycle_PORTB_en;   // input to BRAM
   wire [7:0]                                     error_cycle_PORTB_we;  // input to BRAM
   //  wire        error_cycle_PORTB_clk;// input to BRAM
   //  wire        error_cycle_PORTB_rst;// input to BRAM
   
   (* mark_debug="false" *)wire [31:0] error_vector_PORTB_addr;      // input to BRAM
   (* mark_debug="false" *)wire [31:0] error_vector_PORTB_din;  // input to BRAM
   //(* mark_debug="false" *)wire error_vector_PORTB_en;   // input to BRAM
   (* mark_debug="false" *)wire  [3:0] error_vector_PORTB_we;   // input to BRAM
   //  wire        error_cycle_PORTB_clk;// input to BRAM
   //  wire        error_cycle_PORTB_rst;// input to BRAM
   //----------------------------------------------------------------------------------------------
   //  assign primary_reset = ~primary_reset_out_n ; 
   //  assign any_reset = ( ~peripheral_aresetn | primary_reset ) ;

   (* mark_debug="false" *)wire repeat_counter_done ; 
   (* mark_debug="false" *)wire done_irq ; 

   //(* keep_hierarchy="yes" *)  
   repeat_counter repeat_counter ( 
                                   .clk          (vec_clk), 
                                   .resetn       (vec_clk_aresetn), 

                                   .m_tdata      ( {next_oen, next_dout} ), // in  
                                   .repeat_count ( next_repeat_count ),  // in 
                                   .m_tvalid     (M_AXIS_tvalid),           // in
                                   .m_tready     (M_AXIS_tready),           // out   
                                   .m_tlast      (M_AXIS_tlast),            // in 
      
                                   .s_tdata      ( {oen, dout} ),  // out
                                   .loop_count   () ,    // out
                                   //    .s_tready     (1'b1), //(S_AXIS_tready),  // in
                                   .s_tvalid     (S_AXIS_tvalid),  // out 
                                   .s_tlast      (S_AXIS_tlast),   // out

                                   .done_irq           (repeat_counter_done), // output (for polling  or irq) 
                                   .vec_clk_en         (vec_clk_en)       // output (to gated clk vec_clk); 
                                   //.vec_clk_en_negedge	( )         // out ( gated FCLK1 ) 
                                   ); 

   //----------------------- attach the input data to S2MM input stream ----------
   // (* mark_debug="false" *) assign error  = (oen)&(dout ^ din);
   
   assign next_dout          = M_AXIS_tdata[1*`VECTOR_WIDTH-1:0]; 
   assign next_oen           = M_AXIS_tdata[2*`VECTOR_WIDTH-1:1*`VECTOR_WIDTH]; 
   assign next_repeat_count  = M_AXIS_tdata[`REPEAT_WIDTH+2*`VECTOR_WIDTH-1:2*`VECTOR_WIDTH];

   (* mark_debug="false" *) wire error_detected; // = (error == 0)?1'b0:1'b1; 
   (* mark_debug="false" *) wire count_full = (error_count < `MAX_ERROR_COUNT)?1'b0:1'b1; 
   (* mark_debug="false" *) wire valid_error = (S_AXIS_tvalid & error_detected & ~count_full ) ;
   
   assign error_PORTB_addr = {error_count, 4'b0000}; // lower 4 bits =0 aligns 16 byte boundries
   assign error_PORTB_din  = error;                  // 128 bit wide data 
   assign error_PORTB_we   = (valid_error) ? 16'hffff: 16'h0000; // 16 byte write enable
   assign error_PORTB_en   = valid_error; 
   
   assign error_vector_PORTB_addr = {error_count, 2'b00};   // lower 2 bits=0 alligns to u32 boundries 
   assign error_vector_PORTB_din  = vector_count;           // 32 bit wide data;
   assign error_vector_PORTB_we   = (valid_error) ? 4'hf: 4'h0; // 4 byte write enable
   assign error_vector_PORTB_en   = valid_error;   

   assign error_cycle_PORTB_addr = {error_count, 3'b000}; // lower 3 bits=0 alligns to u64 boundries 
   assign error_cycle_PORTB_din  = cycle_count ;         // 64 bit wide data     
   assign error_cycle_PORTB_we   = (valid_error) ? 8'hff: 8'h00; // 8 byte write enable 
   assign error_cycle_PORTB_en   = valid_error;   

   // io_table is a large mux (no registers) 
   (* mark_debug="false" *)wire [31:0] delay0;
   (* mark_debug="false" *)wire [31:0] delay1; 
   io_table io_table( 
                      .resetn         (vec_clk_aresetn), 
                      .delay_clk      (delay_clk),//FCLK_CLK1),        // 200Mhz phase delay clock 
                      .vec_clk        (vec_clk), 
                      .delay0         (delay0), 
                      .delay1         (delay1),
                      .pulse_ctrl_bits(pulse_ctrl_bits),
                      .vec_clk_90     (vec_clk_90),     // P type
                      .vec_clk_180    (vec_clk_180),    // vec_clk type 
                      .vec_clk_en     (vec_clk_en),     // make vec_clk_en observable on pins (debugging only) 
                      .error_detected (error_detected), // (input)debug scope trigger (this is the Long path) 
                      .pin_type       (pin_type), // from BRAM  (pin_type does not change durring a test) 
                      .dout           (dout),     // input (from stream)
                      .oen            (oen),      // input (from stream) 
                      .error          (error),    // to ERROR RAM and error counter 
                      .pin_din        (pin_din),      // from pin  => negedge register 
                      .pin_oen        (pin_oen),  // to pin
                      .pin_dout       (pin_dout)  // to pin 
                      ); 

   // this defines the mimimnu value for vec_clk relative to FCK_CLK0
   // require that reg_din, oen_reg, and dout_reg must be in the same vec_clk cycle as pin_din, pin_oen, and pin_dout
   // dout_pin,oen_pin change on posedge vec_clk 
   // reg [`VECTOR_WIDTH-1:0] dout_reg; 
   // reg [`VECTOR_WIDTH-1:0] oen_reg ; 
   //always @(*) begin 
   //if (vec_clk_en) begin 
   //      din <= pin_din; // should be stobe clk 
   //dout_reg <= pin_dout; 
   //oen_reg  <= pin_outen; 
   //   end 
   //  end       
   
   genvar                                         n;
   generate
      for (n=0; n<`VECTOR_WIDTH; n=n+1) begin : iobuff0
	      IOBUF iobuf ( 
		                 .IO(gpio[n]),
		                 .O(pin_din[n]),   // output From Pin
		                 .I(pin_dout[n]),  // input  To Pin 
		                 .T(pin_oen[n]));  // input 3-state enable, high=input, low=output
	   end
   endgenerate  

   generate
      for (n=0; n<`EXTRA_GPIO_WIDTH; n=n+1) begin : iobuff1
         IOBUF extra_iobuf ( 
                             .IO(gpio[`VECTOR_WIDTH+n]),
                             .O(extra_gpio_i[n]),  // output From Pin
                             .I(extra_gpio_o[n]),  // input  To Pin 
                             .T(extra_gpio_t[n])    // input 3-state enable, high=input, low=output
                             );
      end
   endgenerate   

   //(* mark_debug="false", ASYNC_REG = "TRUE" *)reg  [`PIN_COUNT-1:0] all_inputs;//= {extra_gpio_i, din}; 
           //(* mark_debug="false" *)reg  [`VECTOR_WIDTH-1:0] all_inputs;//= {extra_gpio_i, din}; 
           //always @(*) all_inputs <= {extra_gpio_i, din}; // this forces input_muxes delay to be < 5 ns and insures debugger will not mangle the input signals. 

   //---------- repeat_counter_done irq/status -----------------------------------
   //wire pinless_extra_o1   = extra_gpio_o[31];     // (from FPGA) unused     
   //wire pinless_extra_t1   = extra_gpio_t[31];     // (from FPGA) unused 
   //assign extra_gpio_i[31] = repeat_counter_done;  // (readonly)  used for repeat done_count done 

   //---------- software triggerr (pinless)  -----------------------------------
   //wire pinless_extra_o0   = extra_gpio_o[30];     // software trigger pin (for  
   //wire pinless_extra_t0   = extra_gpio_t[30];     // (from FPGA) unused  
   //assign extra_gpio_i[30] = extra_gpio_i[30];     // used to trigger freq counters,adc,dac,etc. 

   //-------------------------pin select muxes -----------------------------

   (* mark_debug="false" *)wire [7:0] adc_sel; 
   (* mark_debug="false" *)wire [7:0] ext_adc_sel; 
   (* mark_debug="false" *)wire [7:0] dac_sel ; 
   wire [7:0] unused_sel; 
   (* mark_debug="false" *)wire ADC_trig;
   (* mark_debug="false" *)wire ext_adc_trig;
   (* mark_debug="false" *)wire dac_trig;
   pin_select_mux    adc_mux(  .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(adc_sel[7:0]),      .out(ADC_trig) ); 
   pin_select_mux extadc_mux(  .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(ext_adc_sel[7:0]),  .out(ext_adc_trig) ); 
   pin_select_mux    dac_mux(  .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(dac_sel[7:0]),      .out(dac_trig) ); 

   (* mark_debug="false" *)wire [7:0] pll_en0_sel ; 
   (* mark_debug="false" *)wire [7:0] pll_en1_sel ; 
   (* mark_debug="false" *)wire [7:0] pll_en2_sel ; 
   (* mark_debug="false" *)wire [7:0] pll_en3_sel ; 
   pll_en_select_mux pll_en0_mux( .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(pll_en0_sel[7:0]),  .out(pll_en0) ); 
   pll_en_select_mux pll_en1_mux( .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(pll_en1_sel[7:0]),  .out(pll_en1) );
   pll_en_select_mux pll_en2_mux( .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(pll_en2_sel[7:0]),  .out(pll_en2) ); 
   pll_en_select_mux pll_en3_mux( .clk(FCLK_CLK0), .all_inputs(pin_din), .sel(pll_en3_sel[7:0]),  .out(pll_en3) );

`ifdef USE_SIG_EVENTS
   (* mark_debug="false" *)wire [7:0] sig0_sel; 
   (* mark_debug="false" *)wire [7:0] sig1_sel; 
   (* mark_debug="false" *)wire [7:0] sig2_sel; 
   (* mark_debug="false" *)wire [7:0] sig3_sel; 

   (* mark_debug="false" *)wire sig0_irq;
   (* mark_debug="false" *)wire sig1_irq;
   (* mark_debug="false" *)wire sig2_irq;
   (* mark_debug="false" *)wire sig3_irq;
   pin_select_mux   sig0_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(sig0_sel[7:0]),    .out(sig0_irq) );
   pin_select_mux   sig1_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(sig1_sel[7:0]),    .out(sig1_irq) );
   pin_select_mux   sig2_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(sig2_sel[7:0]),    .out(sig2_irq) );
   pin_select_mux   sig3_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(sig3_sel[7:0]),    .out(sig3_irq) );
`endif 

`ifdef USE_TTC
   pin_select_mux tt0_c1_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(ttc0_clk1_sel[7:0]),  .out(TTC0_CLK1_IN) ); 
   pin_select_mux tt0_c2_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(ttc0_clk2_sel[7:0]),  .out(TTC0_CLK2_IN) );
   pin_select_mux tt1_c1_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(ttc1_clk1_sel[7:0]),  .out(TTC1_CLK1_IN) ); 
   pin_select_mux tt1_c2_mux( .clk(FCLK_CLK0), .all_inputs(all_inputs), .sel(ttc1_clk2_sel[7:0]),  .out(TTC1_CLK2_IN) );
`endif 

   // ------------------ Logic analyzer DMA --------------

   //`ifdef 0 
   //wire [`VECTOR_WIDTH-1:0] analyzer_S_AXIS_tdata = din[`VECTOR_WIDTH-1:0];  //input 
   //wire  [15:0] analyzer_S_AXIS_tkeep = 16'hffff;  //input 
   //wire         analyzer_S_AXIS_tlast;  //input 
   //wire         analyzer_S_AXIS_tready; //output 
   //wire         analyzer_S_AXIS_tvalid; //input 
   //wire [31:0] analyzer_length; 
   //wire [31:0] analyzer_control ; 
   //wire [31:0] analyzer_status ; 
   //`endif 

   //(* keep_hierarchy="yes" *)
   design_1 design_1_i (
                        .DDR_addr   (DDR_addr),
                        .DDR_ba     (DDR_ba),
                        .DDR_cas_n  (DDR_cas_n),
                        .DDR_ck_n   (DDR_ck_n),
                        .DDR_ck_p   (DDR_ck_p),
                        .DDR_cke    (DDR_cke),
                        .DDR_cs_n   (DDR_cs_n),
                        .DDR_dm     (DDR_dm),
                        .DDR_dq     (DDR_dq),
                        .DDR_dqs_n  (DDR_dqs_n),
                        .DDR_dqs_p  (DDR_dqs_p),
                        .DDR_odt    (DDR_odt),
                        .DDR_ras_n  (DDR_ras_n),
                        .DDR_reset_n(DDR_reset_n),
                        .DDR_we_n   (DDR_we_n),
      
                        .FIXED_IO_ddr_vrn   (FIXED_IO_ddr_vrn),
                        .FIXED_IO_ddr_vrp   (FIXED_IO_ddr_vrp),
                        .FIXED_IO_mio       (FIXED_IO_mio),
                        .FIXED_IO_ps_clk    (FIXED_IO_ps_clk),
                        .FIXED_IO_ps_porb   (FIXED_IO_ps_porb),
                        .FIXED_IO_ps_srstb  (FIXED_IO_ps_srstb),
      
                        .FCLK_CLK0          (FCLK_CLK0), // fixed 100Mhz axi_clk 
                        .FCLK_CLK1          (FCLK_CLK1), // Used for phase delays
                        //        .FCLK_CLK2          ( ), // not used 
                        //        .FCLK_CLK3          ( ), // not used 
      
                        .vec_clk            (vec_clk),    // fclk1 derived from FCLK_CLK0 (ungated) 
                        .vec_clk_180        (vec_clk_180),    // pll_clk1 (enables with vec_clk_en) 
                        .vec_clk_90         (vec_clk_90),    // pll_clk2 (enables with vec_clk_en) 
                        .vec_clk_en         (vec_clk_en), // clock enable for BUFGCE (in the pll) 
                        .fclk1_locked       (fclk1_locked), 
                        .pll_clk0           (pll_clk0), 
                        .pll_clk1           (pll_clk1), 
                        .pll_clk2           (pll_clk2),
                        .pll_clk3           (pll_clk3),   
                        .pll_en0            (pll_en0), // clock enable for BUFGCE (in the pll) 
                        .pll_en1            (pll_en1), // clock enable for BUFGCE (in the pll) 
                        .pll_en2            (pll_en2), // clock enable for BUFGCE (in the pll) 
                        .pll_en3            (pll_en3), // clock enable for BUFGCE (in the pll) 
                        .pll_en_sel_tri_o   ({pll_en3_sel[7:0], pll_en2_sel[7:0], pll_en1_sel[7:0], pll_en0_sel[7:0]}), // pinsel[7:0]                      

                        // delay clock
                        .delay_clk          (delay_clk),
                        .delay_clk_locked   (delay_clk_locked),
                        
                        .FCLK_RESET0_N      (FCLK_RESET0_N),
                        //        .FCLK_RESET1_N      ( ),
                        //        .FCLK_RESET2_N      ( ),
                        //        .FCLK_RESET3_N      ( ),
                        .FCLK0_peripheral_aresetn (FCLK0_peripheral_aresetn), // 16 cycle FCLK0 
                        .vec_clk_aresetn          (vec_clk_aresetn),          // 16 cycle fclk1_0 
                        //        .FCLK1_peripheral_aresetn (), // 16 cycle FCLK1  
                        //.dma_reset_out_n          (dma_reset_out_n),   // from dma reset 
      
                        .M_AXIS_tdata     (M_AXIS_tdata),
                        .M_AXIS_tlast     (M_AXIS_tlast),
                        .M_AXIS_tready    (M_AXIS_tready),
                        .M_AXIS_tvalid    (M_AXIS_tvalid),     
      
                        .repeat_count_done_irq_i    (done_irq),// irq indicate everything is done 
      
                        //--------- IO_TABLE AXI Interface ----------------------------
                        .io_table_AXI_awvalid  (io_table_AXI_awvalid), // from master        
                        .io_table_AXI_awready  (io_table_AXI_awready), // to master
                        .io_table_AXI_awaddr   (io_table_AXI_awaddr),  // from master

                        .io_table_AXI_wvalid   (io_table_AXI_wvalid),     // from master
                        .io_table_AXI_wready   (io_table_AXI_wready),     // to master
                        //        .io_table_AXI_wlast    (io_table_AXI_wlast),      // from master   
                        .io_table_AXI_wdata    (io_table_AXI_wdata[31:0]),// from master        
                        .io_table_AXI_wstrb    (io_table_AXI_wstrb[3:0]), // from master
                        
                        .io_table_AXI_bresp    (io_table_AXI_bresp), // to master {  OK, OKEXE, etc.. }          
                        .io_table_AXI_bready   (io_table_AXI_bready),     // from master 
                        .io_table_AXI_bvalid   (io_table_AXI_bvalid),     // to master 
                        
                        .io_table_AXI_arvalid  (io_table_AXI_arvalid),  // from master       
                        .io_table_AXI_arready  (io_table_AXI_arready),  // to   master
                        .io_table_AXI_araddr   (io_table_AXI_araddr),   // from  master 
                        
                        .io_table_AXI_rvalid   (io_table_AXI_rvalid),     // to master   
                        .io_table_AXI_rready   (io_table_AXI_rready),     // from master
                        //        .io_table_AXI_rlast    (io_table_AXI_rlast),      // to master
                        .io_table_AXI_rdata    (io_table_AXI_rdata[31:0]),// to master        
                        .io_table_AXI_rresp    (io_table_AXI_rresp), // to master 
                        
                        //--------- vec_status AXI LITE Interface ----------------------------
                        .vec_status_AXI_awvalid  (vec_status_AXI_awvalid), // from master        
                        .vec_status_AXI_awready  (vec_status_AXI_awready), // to master
                        .vec_status_AXI_awaddr   (vec_status_AXI_awaddr),  // from master

                        .vec_status_AXI_wvalid   (vec_status_AXI_wvalid),     // from master
                        .vec_status_AXI_wready   (vec_status_AXI_wready),     // to master
                        //        .vec_status_AXI_wlast    (vec_status_AXI_wlast),      // from master   
                        .vec_status_AXI_wdata    (vec_status_AXI_wdata[31:0]),// from master        
                        .vec_status_AXI_wstrb    (vec_status_AXI_wstrb[3:0]), // from master
                        
                        .vec_status_AXI_bresp    (vec_status_AXI_bresp), // to master {  OK, OKEXE, etc.. }          
                        .vec_status_AXI_bready   (vec_status_AXI_bready),     // from master 
                        .vec_status_AXI_bvalid   (vec_status_AXI_bvalid),     // to master 
                        
                        .vec_status_AXI_arvalid  (vec_status_AXI_arvalid),  // from master       
                        .vec_status_AXI_arready  (vec_status_AXI_arready),  // to   master
                        .vec_status_AXI_araddr   (vec_status_AXI_araddr),   // from  master 
                        
                        .vec_status_AXI_rvalid   (vec_status_AXI_rvalid),     // to master   
                        .vec_status_AXI_rready   (vec_status_AXI_rready),     // from master
                        //        .vec_status_AXI_rlast    (vec_status_AXI_rlast),      // to master
                        .vec_status_AXI_rdata    (vec_status_AXI_rdata[31:0]),// to master        
                        .vec_status_AXI_rresp    (vec_status_AXI_rresp), // to master 

                        //--------- pulse_ctrl AXI LITE Interface ----------------------------
                        .pulse_ctrl_AXI_awvalid  (pulse_ctrl_AXI_awvalid), // from master        
                        .pulse_ctrl_AXI_awready  (pulse_ctrl_AXI_awready), // to master
                        .pulse_ctrl_AXI_awaddr   (pulse_ctrl_AXI_awaddr),  // from master

                        .pulse_ctrl_AXI_wvalid   (pulse_ctrl_AXI_wvalid),     // from master
                        .pulse_ctrl_AXI_wready   (pulse_ctrl_AXI_wready),     // to master
                        //        .pulse_ctrl_AXI_wlast    (pulse_ctrl_AXI_wlast),      // from master   
                        .pulse_ctrl_AXI_wdata    (pulse_ctrl_AXI_wdata[31:0]),// from master        
                        .pulse_ctrl_AXI_wstrb    (pulse_ctrl_AXI_wstrb[3:0]), // from master
                        
                        .pulse_ctrl_AXI_bresp    (pulse_ctrl_AXI_bresp), // to master {  OK, OKEXE, etc.. }          
                        .pulse_ctrl_AXI_bready   (pulse_ctrl_AXI_bready),     // from master 
                        .pulse_ctrl_AXI_bvalid   (pulse_ctrl_AXI_bvalid),     // to master 
                        
                        .pulse_ctrl_AXI_arvalid  (pulse_ctrl_AXI_arvalid),  // from master       
                        .pulse_ctrl_AXI_arready  (pulse_ctrl_AXI_arready),  // to   master
                        .pulse_ctrl_AXI_araddr   (pulse_ctrl_AXI_araddr),   // from  master 
                        
                        .pulse_ctrl_AXI_rvalid   (pulse_ctrl_AXI_rvalid),     // to master   
                        .pulse_ctrl_AXI_rready   (pulse_ctrl_AXI_rready),     // from master
                        //        .pulse_ctrl_AXI_rlast    (pulse_ctrl_AXI_rlast),      // to master
                        .pulse_ctrl_AXI_rdata    (pulse_ctrl_AXI_rdata[31:0]),// to master        
                        .pulse_ctrl_AXI_rresp    (pulse_ctrl_AXI_rresp), // to master 
                        
                        //--------- freq counter AXI LITE Interface ----------------------------
                        .freq_AXI_awvalid  (freq_AXI_awvalid), // from master        
                        .freq_AXI_awready  (freq_AXI_awready), // to master
                        .freq_AXI_awaddr   (freq_AXI_awaddr),  // from master

                        .freq_AXI_wvalid   (freq_AXI_wvalid),     // from master
                        .freq_AXI_wready   (freq_AXI_wready),     // to master
                        //        .freq_AXI_wlast    (freq_AXI_wlast),      // from master   
                        .freq_AXI_wdata    (freq_AXI_wdata[31:0]),// from master        
                        .freq_AXI_wstrb    (freq_AXI_wstrb[3:0]), // from master
                        
                        .freq_AXI_bresp    (freq_AXI_bresp), // to master {  OK, OKEXE, etc.. }          
                        .freq_AXI_bready   (freq_AXI_bready),     // from master 
                        .freq_AXI_bvalid   (freq_AXI_bvalid),     // to master 
                        
                        .freq_AXI_arvalid  (freq_AXI_arvalid),  // from master       
                        .freq_AXI_arready  (freq_AXI_arready),  // to   master
                        .freq_AXI_araddr   (freq_AXI_araddr),   // from  master 
                        
                        .freq_AXI_rvalid   (freq_AXI_rvalid),     // to master   
                        .freq_AXI_rready   (freq_AXI_rready),     // from master
                        //        .freq_AXI_rlast    (freq_AXI_rlast),      // to master
                        .freq_AXI_rdata    (freq_AXI_rdata[31:0]),// to master        
                        .freq_AXI_rresp    (freq_AXI_rresp), // to master 
                        
                        .freq_counter_irq_i (freq_irq),  // to cpu  
                        
                        //-----------------------------------------------------------------------    
                        .error_PORTB_addr   (error_PORTB_addr), // input [31:0] to BRAM
                        .error_PORTB_din    (error_PORTB_din),  // input[127:0] to BRAM 
                        .error_PORTB_dout   (), // output[127:0] from BRAM 
                        .error_PORTB_en     (error_PORTB_en),   // input to  BRAM 
                        .error_PORTB_we     (error_PORTB_we),   // input [15:0] to BRAM 
                        .error_PORTB_clk    (vec_clk),        // input to BRAM 
                        .error_PORTB_rst    (~vec_clk_aresetn),  // input to BRAM 
                        
                        .error_vector_PORTB_addr   (error_vector_PORTB_addr), // input [31:0] to BRAM
                        .error_vector_PORTB_din    (error_vector_PORTB_din),  // input [31:0] to BRAM 
                        .error_vector_PORTB_dout   (), // output[31:0] from BRAM 
                        .error_vector_PORTB_en     (error_vector_PORTB_en),   // input to  BRAM 
                        .error_vector_PORTB_we     (error_vector_PORTB_we),   // input [3:0] to BRAM 
                        .error_vector_PORTB_clk    (vec_clk),              // input to BRAM 
                        .error_vector_PORTB_rst    (~vec_clk_aresetn),    // input to BRAM 
                        
                        .error_cycle_PORTB_addr   (error_cycle_PORTB_addr), // input [31:0] to BRAM
                        .error_cycle_PORTB_din    (error_cycle_PORTB_din),  // input [63:0] to BRAM 
                        .error_cycle_PORTB_dout   (), // output[63:0] from BRAM 
                        .error_cycle_PORTB_en     (error_cycle_PORTB_en),   // input to  BRAM 
                        .error_cycle_PORTB_we     (error_cycle_PORTB_we),   // input [7:0] to BRAM 
                        .error_cycle_PORTB_clk    (vec_clk),              // input to BRAM 
                        .error_cycle_PORTB_rst    (~vec_clk_aresetn),    // input to BRAM 
                        
                        //-----------------------------------------------------------------------     
                        .extra_gpio_tri_o     (extra_gpio_o), // output from fpga 
                        .extra_gpio_tri_i     (extra_gpio_i), // from pins 
                        .extra_gpio_tri_t     (extra_gpio_t), // active low output enable 
                        
                        //-----------------------------------------------------------------------    
                        .adc_dac_sel_tri_o  ({unused_sel[7:0],dac_sel[7:0],ext_adc_sel[7:0],adc_sel[7:0]}), //  pinsel[7:0]
                        .irq_dac_i          (dac_trig),
                        .irq_ext_adc_i      (ext_adc_trig), 
                        
`ifdef USE_SIG_EVENTS
                        .sig_sel_tri_o  ({sig3_sel[7:0], sig2_sel[7:0], sig1_sel[7:0], sig0_sel[7:0]}), // pinsel[7:0]              
                        .irq_sig0_i     (sig0_irq),
                        .irq_sig1_i     (sig1_irq), 
                        .irq_sig2_i     (sig2_irq), 
                        .irq_sig3_i     (sig3_irq), 
`endif 

                        // -------------------------------------------------------------------------------
                        //            8 frequency counters 
                        // -------------------------------------------------------------------------------
`ifdef USE_TTC 
                        .TTC_clk_sel_tri_o   ({ ttc1_clk2_sel[7:0], ttc1_clk1_sel[7:0], ttc0_clk2_sel[7:0], ttc0_clk1_sel[7:0]}),

                        .TTC_OUT               (TTC_OUT[5:0]),
                        .TTC0_CLK1_IN          (TTC0_CLK1_IN),
                        .TTC0_CLK2_IN          (TTC0_CLK2_IN),
                        .TTC1_CLK1_IN          (TTC1_CLK1_IN),
                        .TTC1_CLK2_IN          (TTC1_CLK2_IN),      
`endif           
                        .ADC_trig      (ADC_trig),
                        .Vaux0_v_n     (Vaux_v_n[0]),
                        .Vaux0_v_p     (Vaux_v_p[0]),
                        .Vaux1_v_n     (Vaux_v_n[1]),
                        .Vaux1_v_p     (Vaux_v_p[1]),
                        .Vaux2_v_n     (Vaux_v_n[2]),
                        .Vaux2_v_p     (Vaux_v_p[2]),
                        .Vaux3_v_n     (Vaux_v_n[3]),
                        .Vaux3_v_p     (Vaux_v_p[3]),
                        .Vaux4_v_n     (Vaux_v_n[4]),
                        .Vaux4_v_p     (Vaux_v_p[4]),
                        .Vaux5_v_n     (Vaux_v_n[5]),
                        .Vaux5_v_p     (Vaux_v_p[5]),
                        .Vaux6_v_n     (Vaux_v_n[6]),
                        .Vaux6_v_p     (Vaux_v_p[6]),
                        .Vaux7_v_n     (Vaux_v_n[7]),
                        .Vaux7_v_p     (Vaux_v_p[7]),
                        .Vaux8_v_n     (Vaux_v_n[8]),
                        .Vaux8_v_p     (Vaux_v_p[8]),
                        .Vaux9_v_n     (Vaux_v_n[9]),
                        .Vaux9_v_p     (Vaux_v_p[9]),
                        .Vaux10_v_n    (Vaux_v_n[10]),
                        .Vaux10_v_p    (Vaux_v_p[10]),
                        .Vaux11_v_n    (Vaux_v_n[11]),
                        .Vaux11_v_p    (Vaux_v_p[11]),
                        .Vaux12_v_n    (Vaux_v_n[12]),
                        .Vaux12_v_p    (Vaux_v_p[12]),
                        .Vaux13_v_n    (Vaux_v_n[13]),
                        .Vaux13_v_p    (Vaux_v_p[13]),
                        .Vaux14_v_n    (Vaux_v_n[14]),
                        .Vaux14_v_p    (Vaux_v_p[14]),
                        .Vaux15_v_n    (Vaux_v_n[15]),
                        .Vaux15_v_p    (Vaux_v_p[15]),
                        
                        .Vp_Vn_v_n     (Vp_Vn_v_n),
                        .Vp_Vn_v_p     (Vp_Vn_v_p)

                        );
   
   // ------------------------------------------------------------------------------
   //          IO_Table AXI CR register bank 
   // -------------------------------------------------------------------------------
   axi_io_table axi_io_table_inst ( 
                                    .control_bits   (pin_type), // 512 output control bits. 
                                    .delay0         (delay0[31:0]), 
                                    .delay1         (delay1[31:0]), 
	                                 //---- write transfer 
                                    .awaddr    (io_table_AXI_awaddr[13:0]), 
                                    .awlen     ('h0),//(io_table_AXI_awlen),   // wvalids/awvalid count = [0..len-1]  
                                    .awsize    ('h2), //(io_table_AXI_awsize),       // {1,2,4,8,16,32,64,128} BYTES/wvalid 
                                    .awburst   ('h1), //(io_table_AXI_awburst), // {FIXED, INCR, WRAP, reserved} 
                                    .awvalid   (io_table_AXI_awvalid), 
                                    .awready   (io_table_AXI_awready),

	                                 .wdata     (io_table_AXI_wdata[31:0]), 
                                    .wstrb     (io_table_AXI_wstrb[3:0]), 
                                    .wvalid    (io_table_AXI_wvalid), 
                                    .wready    (io_table_AXI_wready), 
                                    .wlast     (1'b1),//(io_table_AXI_wlast), 
	                                 // wid, wuser 
      
                                    .bresp     (io_table_AXI_bresp),// to master {OK, OKEXT, SLVERR, DECERR}  
                                    .bvalid    (io_table_AXI_bvalid),    // to master
                                    .bready    (io_table_AXI_bready),    // from master
      
	                                 //---- read transfer 
                                    .araddr    (io_table_AXI_araddr[13:0]),
                                    .arlen     ('h0),//(io_table_AXI_arlen),   // from master  rvalids/arvalid   
                                    .arsize    ('h2), //(io_table_AXI_arsize),       // from master {1,2,4,8,16,32,64,128} BYTES/rvalid 
                                    .arburst   ('h1),//(io_table_AXI_arburst), // from master {FIXED, INCR, WRAP, reserved} 
                                    .arvalid   (io_table_AXI_arvalid),      // from master 
                                    .arready   (io_table_AXI_arready),      // to master 
      
                                    .rdata     (io_table_AXI_rdata[31:0]), // to master  
                                    .rresp     (io_table_AXI_rresp),  // to master {OK, OKEXT, SLVERR, DECERR} 
                                    .rvalid    (io_table_AXI_rvalid),      // to master  
                                    .rready    (io_table_AXI_rready),      // from master  
                                    .rlast     (io_table_AXI_rlast),       // to master (last rvalid)  
	                                 // rid, ruser 

                                    .clk0      (vec_clk),            //(FCLK_CLK0), 
                                    .resetn    (vec_clk_aresetn)); //(FCLK0_peripheral_aresetn) ) ; 

   // ------------------------------------------------------------------------------
   //          Pulse_ctrl AXI CR register bank 
   // -------------------------------------------------------------------------------
   axi_pulse_ctrl axi_pulse_ctrl_inst ( 
                                        .pulse_ctrl_bits   (pulse_ctrl_bits), // 2048 output control bits. 
	                                     //---- write transfer 
                                        .awaddr    (pulse_ctrl_AXI_awaddr[13:0]), 
                                        .awlen     ('h0),//(pulse_ctrl_AXI_awlen),   // wvalids/awvalid count = [0..len-1]  
                                        .awsize    ('h2), //(pulse_ctrl_AXI_awsize),       // {1,2,4,8,16,32,64,128} BYTES/wvalid 
                                        .awburst   ('h1), //(pulse_ctrl_AXI_awburst), // {FIXED, INCR, WRAP, reserved} 
                                        .awvalid   (pulse_ctrl_AXI_awvalid), 
                                        .awready   (pulse_ctrl_AXI_awready),

	                                     .wdata     (pulse_ctrl_AXI_wdata[31:0]), 
                                        .wstrb     (pulse_ctrl_AXI_wstrb[3:0]), 
                                        .wvalid    (pulse_ctrl_AXI_wvalid), 
                                        .wready    (pulse_ctrl_AXI_wready), 
                                        .wlast     (1'b1),//(pulse_ctrl_AXI_wlast), 
	                                     // wid, wuser 
      
                                        .bresp     (pulse_ctrl_AXI_bresp),// to master {OK, OKEXT, SLVERR, DECERR}  
                                        .bvalid    (pulse_ctrl_AXI_bvalid),    // to master
                                        .bready    (pulse_ctrl_AXI_bready),    // from master
      
	                                     //---- read transfer 
                                        .araddr    (pulse_ctrl_AXI_araddr[13:0]),
                                        .arlen     ('h0),//(pulse_ctrl_AXI_arlen),   // from master  rvalids/arvalid   
                                        .arsize    ('h2), //(pulse_ctrl_AXI_arsize),       // from master {1,2,4,8,16,32,64,128} BYTES/rvalid 
                                        .arburst   ('h1),//(pulse_ctrl_AXI_arburst), // from master {FIXED, INCR, WRAP, reserved} 
                                        .arvalid   (pulse_ctrl_AXI_arvalid),      // from master 
                                        .arready   (pulse_ctrl_AXI_arready),      // to master 
      
                                        .rdata     (pulse_ctrl_AXI_rdata[31:0]), // to master  
                                        .rresp     (pulse_ctrl_AXI_rresp),  // to master {OK, OKEXT, SLVERR, DECERR} 
                                        .rvalid    (pulse_ctrl_AXI_rvalid),      // to master  
                                        .rready    (pulse_ctrl_AXI_rready),      // from master  
                                        .rlast     (pulse_ctrl_AXI_rlast),       // to master (last rvalid)  
	                                     // rid, ruser 

                                        .clk0      (vec_clk),            //(FCLK_CLK0), 
                                        .resetn    (vec_clk_aresetn)); //(FCLK0_peripheral_aresetn) ) ; 
   
   //-------------------------------------------------------------------------------
   //       8 freequency counters 
   //-------------------------------------------------------------------------------

   axi_freq_counter axi_freq_counter_inst ( 
                                            .all_inputs ({extra_gpio_i, pin_din}),
                                            .irq        (freq_irq), 
      
                                            .awaddr     (freq_AXI_awaddr[13:0]), 
                                            .awvalid    (freq_AXI_awvalid), 
                                            .awready    (freq_AXI_awready), 
                                            .awlen      ('h0), 
                                            .awsize     ('h2), 
                                            .awburst    ('h1),
      
                                            .araddr     (freq_AXI_araddr[13:0]), 
                                            .arvalid    (freq_AXI_arvalid), 
                                            .arready    (freq_AXI_arready), 
                                            .arlen      ('h0), 
                                            .arsize     ('h2), 
                                            .arburst    ('h1),
      
                                            .rdata      (freq_AXI_rdata), 
                                            .rresp      (freq_AXI_rresp), 
                                            .rvalid     (freq_AXI_rvalid), 
                                            .rready     (freq_AXI_rready), 
                                            .rlast      (freq_AXI_rlast), // rid, ruser 
      
                                            .wdata      (freq_AXI_wdata), 
                                            .wstrb      (freq_AXI_wstrb), 
                                            .wvalid     (freq_AXI_wvalid), 
                                            .wready     (freq_AXI_wready), 
                                            .wlast      (1'b1), // wid, wuser 
      
                                            .bresp      (freq_AXI_bresp), 
                                            .bvalid     (freq_AXI_bvalid), 
                                            .bready     (freq_AXI_bready),  
      
                                            .clk0      (FCLK_CLK0), 
                                            .resetn    (FCLK0_peripheral_aresetn) 
                                            ) ; 

   //-------------------------------------------------------------------------------
   //   vector status registers  
   //-------------------------------------------------------------------------------

   axi_vector_status axi_vector_status_inst ( 
      
                                              .final_gap_count    (final_gap_count), 
                                              .final_error_count  (final_error_count), 
                                              .final_vector_count (final_vector_count),
                                              .final_cycle_count  (final_cycle_count), 
                                              .errors_detected    (first_error_detected),
      
                                              .repeat_count_done  (repeat_counter_done), // input (for pooling or irq) 
                                              .done_irq           (done_irq), // output to cpu 
      
                                              .awaddr     (vec_status_AXI_awaddr[13:0]), 
                                              .awvalid    (vec_status_AXI_awvalid), 
                                              .awready    (vec_status_AXI_awready), 
                                              .awlen      ('h0), 
                                              .awsize     ('h2), 
                                              .awburst    ('h1),
      
                                              .araddr     (vec_status_AXI_araddr[13:0]), 
                                              .arvalid    (vec_status_AXI_arvalid), 
                                              .arready    (vec_status_AXI_arready), 
                                              .arlen      ('h0), 
                                              .arsize     ('h2), 
                                              .arburst    ('h1),
      
                                              .rdata      (vec_status_AXI_rdata), 
                                              .rresp      (vec_status_AXI_rresp), 
                                              .rvalid     (vec_status_AXI_rvalid), 
                                              .rready     (vec_status_AXI_rready), 
                                              .rlast      (vec_status_AXI_rlast), // rid, ruser 
      
                                              .wdata      (vec_status_AXI_wdata), 
                                              .wstrb      (vec_status_AXI_wstrb), 
                                              .wvalid     (vec_status_AXI_wvalid), 
                                              .wready     (vec_status_AXI_wready), 
                                              .wlast      (1'b1), // wid, wuser 
      
                                              .bresp      (vec_status_AXI_bresp), 
                                              .bvalid     (vec_status_AXI_bvalid), 
                                              .bready     (vec_status_AXI_bready),  
      
                                              .clk0      (vec_clk), //(FCLK_CLK0), 
                                              .resetn    (vec_clk_aresetn)//(FCLK0_peripheral_aresetn) 
                                              ) ; 

   //-------------------------------------------------------------------------------
   //(* keep_hierarchy="yes" *)  
   error_counter ERROR_counter ( 
                                 .clk                (vec_clk), 
                                 .resetn             (vec_clk_aresetn), 
                                 .m_tvalid           (M_AXIS_tvalid),          // input 
                                 .m_tready           (M_AXIS_tready),          // input 
                                 .m_tlast            (M_AXIS_tlast),           // input 
      
                                 .vec_clk_en         (vec_clk_en),           // input  indicates valid errors 
                                 .error              (error[`VECTOR_WIDTH-1:0]),    // input [n:0] 
                                 .s_tvalid           (S_AXIS_tvalid),   // input 
                                 .s_tlast            (S_AXIS_tlast),    // input 
                                 //        .s_tready           (S_AXIS_tready),    // input 
      
                                 .error_detected     (error_detected),   // output 
                                 .vector_count       (vector_count),     // output [31:0] 
                                 .cycle_count        (cycle_count),      // output [63:0] 
                                 .error_count        (error_count),      // output [10:0]
      
                                 .first_error_detected (first_error_detected), // output to vector status 
                                 .final_gap_count    (final_gap_count),    // output [31:0]
                                 .final_error_count  (final_error_count),  // output [10:0]    
                                 .final_vector_count (final_vector_count), // output [31:0]
                                 .final_cycle_count  (final_cycle_count)   // output [63:0] 
                                 ) ;        
   
   //----------------------------------------------------------------------------        
   /*
    STS_module STS( 
    .clk    (FCLK_CLK1), 
    .reset  (any_reset), 
    .last_cycle (test_last_cycle),  // input indicates endof test   
    
    .s_tdata  (S_AXIS_STS_tdata),   // output [31:0] 
    .s_tvalid (S_AXIS_STS_tvalid),  // output 
    .s_tlast  (S_AXIS_STS_tlast),   // output
    .s_tready (S_AXIS_STS_tready),  // input 

    .user0  (final_gap_count) ,     // input [31:0]
    .user1  (final_error_count),    // input [31:0] 
    .user2  (first_error_vector),   // input [31:0]
    .user3  (last_error_vector),    // input [31:0]
    .user4  (final_vector_count)    // input [31:0]
    ); 
    */        
   //----------------------------------------------------------------------------------------
   //(* DONT_TOUCH = "yes" *) 
   //(* keep_hierarchy="yes" *)  
   /*
    CTRL_module CTRL_0(
    .clk  (FCLK_CLK1), 
    .reset (any_reset), 
    .m_tdata  (M_AXIS_CNTRL_tdata[31:0]),   // input [31:0] 
    .m_tvalid (M_AXIS_CNTRL_tvalid),  // input 
    .m_tlast  (M_AXIS_CNTRL_tlast),   // input
    .m_tready (M_AXIS_CNTRL_tready),  // output 
    .user0  (cntrl_0_total_packet_count),  // output [31:0]
    .user1  (cntrl_0_total_byte_count),    // output [31:0]
    .uservoid print_freq_status(freq_status_t status)2  (cntrl_0_total_repeat_count),  // output [31:0] 
    .user3  (cntrl_0_cycle_period_ps),     // output [31:0]
    .user4  (cntrl_0_total_cycle_count)    // output [31:0]
    ); 
    
    //(* DONT_TOUCH = "yes", keep_hierarchy="yes" *) 
    //(* keep_hierarchy="yes" *) 
    CTRL_module CTRL_1(
    .clk    (FCLK_CLK1), 
    .reset  (any_reset), 
    .m_tdata  (M_AXIS_CNTRL_tdata[63:32]),   // input [31:0] 
    .m_tvalid (M_AXIS_CNTRL_tvalid),  // input 
    .m_tlast  (M_AXIS_CNTRL_tlast),
    .m_tready (),  // output 
    .user0  (cntrl_1_total_packet_count),  // output [31:0]
    .user1  (cntrl_1_total_byte_count),    // output [31:0]
    .user2  (cntrl_1_total_repeat_count),  // output [31:0] 
    .user3  (cntrl_1_cycle_period_ps),     // output [31:0]
    .user4  (cntrl_1_total_cycle_count)    // output [31:0]
    ); 

    //(* DONT_TOUCH = "yes", keep_hierarchy="yes" *)  
    CTRL_module CTRL_2(
    .clk        (FCLK_CLK1),
    .reset      (any_reset), 
    .m_tdata    (M_AXIS_CNTRL_tdata[95:64]),   // input [31:0] 
    .m_tvalid   (M_AXIS_CNTRL_tvalid),  // input 
    .m_tlast    (M_AXIS_CNTRL_tlast),
    .m_tready   (),  // output 
    .user0  (cntrl_2_total_packet_count),  // output [31:0]
    .user1  (cntrl_2_total_byte_count),    // output [31:0]
    .user2  (cntrl_2_total_repeat_count),  // output [31:0] 
    .user3  (cntrl_2_cycle_period_ps),     // output [31:0]
    .user4  (cntrl_2_total_cycle_count)    // output [31:0]
    );         
    */       
endmodule  // end of top module 

//----------------------------------------------------------------------------------

module pll_en_select_mux(clk,  all_inputs, sel, out ) ; 
   input [`VECTOR_WIDTH-1:0] all_inputs ; 
   input [7:0]               sel;             
   output reg                out; 
   input                     clk ; 
   //   (* ASYNC_REG="TRUE" *)reg out; // do not resource share this reg
   //(* keep="true" *)reg out; // do not resource share this reg
   //    wire [7:0] sub_set = {3'b100, sel[4:0]}; // top 32 bits implies Extra_GPIOs
   //    wire [7:0] sub_set = sel[7:0];
   //    assign out = (sel<`VECTOR_WIDTH)  ? all_inputs[sel] : 1'b1; 
   always @(*)  begin 
      if (sel<`VECTOR_WIDTH) 
        out <= all_inputs[sel];
      else if (sel == 8'hfe) 
        out <= 1'b0; // disable
      else 
        out <= 1'b1; // enabled
   end 
endmodule  
//----------------------------------------------------------------------------------

module pin_select_mux(clk,  all_inputs, sel, out ) ; 
   input [`VECTOR_WIDTH-1:0] all_inputs ; 
   input [7:0]               sel;             
   output reg                out; 
   input                     clk ; 
   //   (* ASYNC_REG="TRUE" *)reg out; // do not resource share this reg 
   //    wire [7:0] sub_set = {3'b100, sel[4:0]}; // top 32 bits implies Extra_GPIOs
   //    wire [7:0] sub_set = sel[7:0];
   //    assign out = (sel<`VECTOR_WIDTH)  ? all_inputs[sel] : 1'b0; 
   //    always @(posedge clk)  begin 
   always @(*)  begin 
      if (sel<`VECTOR_WIDTH) 
        out <= all_inputs[sel];
      else if (sel == 8'hfe) 
        out <= 1'b1; // disable
      else 
        out <= 1'b0; // disabled 
   end 
endmodule  

//---------------------------------------------------------------------------------- 
`ifndef HOLD_TIME 
 `define HOLD_TIME 0 
`endif       
//(* DONT_TOUCH="yes", keep_hierarchy="yes" *)
module repeat_counter ( 
                        clk, resetn, vec_clk_en, //vec_clk_en_negedge,
                        m_tvalid, m_tready, m_tlast,  m_tdata, repeat_count, 
                        done_irq,    
                        //    s_tready, 
                        s_tvalid, s_tlast,  s_tdata,
                        loop_count 
                        ); 
   
   input wire clk; 
   input wire resetn; 
   input  wire [`REPEAT_WIDTH-1:0] repeat_count; 
   input  wire [2*`VECTOR_WIDTH-1:0] m_tdata; 
   input  wire                       m_tvalid; 
   output wire                       m_tready ; 
   input  wire                       m_tlast; 
   
   // output reg  vec_clk_en_negedge;   // used for clock gating 
                                     output wire vec_clk_en;   // used for clock gating 

   output reg [`REPEAT_WIDTH-1:0]                loop_count; 
   output reg [2*`VECTOR_WIDTH-1:0]              s_tdata; 
   output wire                                   s_tvalid; 
   //  input  wire s_tready; 
   output wire                                   s_tlast; 

   output reg                                    done_irq; // set when last cycle occurs, clears on the first cycle. 

   //-----------------------------------------------------------------------
   
   (* mark_debug="false" *) wire loop_count_zero ; 
   (* mark_debug="false" *) wire pop_m ; 

   (* mark_debug="false" *) reg s_cycle ;
   (* mark_debug="false" *) reg last_cycle ; 

   //------------------------------------------------------------------------------
   // with out repeat count : 
   // {s_tdata, s_tlast, s_tvalid} are {m_tdata, m_tlast, m_tvalid} delayed by one cycle 
   // This allows registers on DIN 
   //  excpet when loop count not equal zero 

   assign loop_count_zero  = (loop_count == 0) ; 
   assign m_tready         = loop_count_zero; 
   assign pop_m            = (m_tvalid & m_tready);
   
   // repeat vectors are not pushed to S_TDATA fifo. 
   always @(posedge clk ) begin 
      if (~resetn) 
        loop_count <= #(`HOLD_TIME) 0; 
      else if (pop_m) 
        loop_count <= #(`HOLD_TIME) repeat_count; 
      else if (~loop_count_zero) 
        loop_count <= #(`HOLD_TIME) loop_count-1; 
   end      
   
   always @(posedge clk ) begin  
      if (~resetn) 
        s_cycle <= #(`HOLD_TIME) 0 ; 
      else if (pop_m) 
        s_cycle <= #(`HOLD_TIME) 1; 
      else if (loop_count_zero) 
        s_cycle <= #(`HOLD_TIME) 0; 
   end 
   
   always @(posedge clk ) begin  
      if (~resetn) 
        last_cycle <= #(`HOLD_TIME) 0 ; 
      else if ( m_tlast & pop_m) 
        last_cycle <= #(`HOLD_TIME) 1; 
      else if (last_cycle & loop_count_zero ) 
        last_cycle <= #(`HOLD_TIME) 0; 
   end 

   always @(posedge clk ) begin 
      if (~resetn) 
        s_tdata  <= #(`HOLD_TIME){ {`VECTOR_WIDTH{1'b1}},{`VECTOR_WIDTH{1'b0}} }; // reset value is "outputs are undriven"  
      else if (pop_m) 
        s_tdata  <= #(`HOLD_TIME)m_tdata; 
   end    
   
   // negedge vec_clk_en eliminates  false posedge on gated clock ( clk & en)        
   // (m_tvalid == 1) & (s_tready == 0) indicate a error_fifo full condition 
   // (m_tvalid == 0) & (s_tready == 1) indicates data_fifo empty condition) 
   //    assign vec_clk_en = s_cycle & s_tready;
   assign vec_clk_en = s_cycle ;
   
   assign s_tvalid = s_cycle    & loop_count_zero ;
   assign s_tlast  = last_cycle & loop_count_zero ; 

   // done state machine 
   // set on last cycle 
   // clear on first cycle 
   
   always @(posedge clk ) begin 
      if (~resetn) 
        done_irq  <= #(`HOLD_TIME) 1; // resets to done state 
      else if (s_tlast)   
        done_irq <= #(`HOLD_TIME) 1;  
      else if (done_irq & pop_m) // first 
 	     done_irq  <= #(`HOLD_TIME) 0;
   end          

endmodule    

//=========================================================================================== 
//(* DONT_TOUCH = "yes", keep_hierarchy="yes" *) 
module error_counter( clk, resetn,

                      m_tvalid, m_tready, m_tlast,  
                      error, s_tvalid, s_tlast, //s_tready, 
                      vec_clk_en, error_detected, 
                      error_count, vector_count, cycle_count,  
                      first_error_detected, 
                      final_gap_count, final_error_count, 
                      final_vector_count,
                      final_cycle_count ) ; 

   input wire clk; 
   input wire resetn; 
   input wire m_tvalid; 
   input wire m_tready; 
   input wire m_tlast; 

   input wire vec_clk_en;             // indicates repeat counts. 
   input wire [`VECTOR_WIDTH-1:0] error; 
   input wire                     s_tvalid; 
   //    input wire s_tready; 
   input wire                     s_tlast; 
   
   output reg [31:0]              final_gap_count; 
   output reg [`ERROR_COUNT_WIDTH:0] final_error_count; 
   output reg [31:0]                 final_vector_count; 
   output reg [63:0]                 final_cycle_count;     
   output                            first_error_detected; 
   
   output                            error_detected; 
   output reg [`ERROR_COUNT_WIDTH:0] error_count ; 
   output reg [31:0]                 vector_count ;   
   output [63:0]                     cycle_count ;   
   
   (* mark_debug="true" *)reg [31:0] gap_count ;  
   (* mark_debug="true" *)reg [63:0] cycle_count ;    
   (* mark_debug="false" *)wire gap_detected;
   (* mark_debug="false" *)reg first_error_detected;
   (* mark_debug="false" *)wire error_detected;       
   (* mark_debug="false" *)reg state; 
   wire                              next_state; 
   
`define ERR_IDLE  0 
`define ERR_RUN   1 

   (* mark_debug="false" *)wire first = ((state == `ERR_IDLE) & m_tvalid & m_tready); 
   (* mark_debug="false" *)wire last  = ((state == `ERR_RUN)  & s_tlast & s_tvalid );    
   assign next_state = 
                       ((state == `ERR_IDLE) & m_tvalid & m_tready) ? `ERR_RUN : 
                       ((state == `ERR_RUN)  & s_tlast)     ? `ERR_IDLE  :       
                       state ;  
   
   always @(posedge clk ) begin    
      if ( ~resetn )
        state <= `ERR_IDLE; 
      else
        state <= next_state; 
   end  
   
   always @(posedge clk ) begin    
      if ( ~resetn | first  )
        cycle_count <= 64'h0; 
      else if (vec_clk_en)
        cycle_count <= cycle_count +64'h1; 
   end 
   
   always @(posedge clk ) begin    
      if ( ~resetn | first )
        vector_count <= 32'h0; 
      else if (s_tvalid) 
        vector_count <= vector_count +32'h1; 
   end 
   
   // (m_tvalid == 1) & (s_tready == 0) indicate a error_fifo full condition 
   // (m_tvalid == 0) & (s_tready == 1) indicates data_fifo empty condition) 
   assign gap_detected = (state==`ERR_RUN) & ~vec_clk_en;// & test_out_en; 
   
   always @(posedge clk ) begin    
      if (first | ~resetn )
        gap_count <= 32'h0; 
      else if (gap_detected ) 
        gap_count <= gap_count +32'h1; 
   end 
   
   assign error_detected       = (!(error == 'h0) & s_tvalid ); 
   //assign first_error_detected = error_detected & (error_count == 'h0);
   always @(posedge clk) begin  
      if (~resetn | first )
        first_error_detected <= 1'b0; 
      //else if (first & ~error_detected) // first(master) and error(slave) can never occure 
      //     first_error_detected <= 1'b0;     
      else if (error_detected)
        first_error_detected <= 1'b1; 
   end 
   
   always @(posedge clk ) begin    
      if (first  | ~resetn )
        error_count <= 'h0; 
      else if (error_detected)
        error_count <= error_count + 'h1; 
   end 

   always @(posedge clk ) begin    
      if (~resetn) begin 
         final_vector_count <= 'h0; 
         final_gap_count    <= 'h0; 
         final_error_count  <= 'h0; 
         final_cycle_count  <= 'h0; 
      end 
      else if (s_tlast) begin
         final_vector_count <= vector_count; 
         final_gap_count    <= gap_count; 
         final_error_count  <= error_count; 
         final_cycle_count  <= cycle_count; 
      end 
   end 

endmodule 

//----------------------------------------------------------------------------------
`ifdef USE_STS_CTS

//(* DONT_TOUCH = "yes", keep_hierarchy="yes" *) 
module STS_module( clk, reset, 
                   last_cycle, 
                   s_tdata, s_tlast, s_tready, s_tvalid,
                   user0, user1, user2, user3, user4 ) ;
   input clk; 
   input reset;
   input last_cycle;  
   
   output [31:0] s_tdata;  // S_AXIS_STS_tdata;//  = M_AXIS_CNTRL_tdata;
                 output        s_tlast;  //S_AXIS_STS_tlast;//  = M_AXIS_CNTRL_tlast;
                               input         s_tready; // S_AXIS_STS_tready;
   output                                    s_tvalid; //  S_AXIS_STS_tvalid;//  = M_AXIS_CNTRL_tvalid;
   
   input [31:0]                              user0; 
   input [31:0]                              user1; 
   input [31:0]                              user2; 
   input [31:0]                              user3; 
   input [31:0]                              user4;
   
   reg [3:0]                                 sts_state ; 

   always @(posedge clk or posedge reset ) begin    
      if (reset) 
        sts_state <= 'hc; 
      else if (last_cycle) 
        sts_state <= 'b0; 
      else if (s_tready && (sts_state < 'hc))
        sts_state <= sts_state +'b1;
   end 

   assign s_tdata = 
                    (sts_state == 'h0)  ? 32'h5000_0000       : // sts_flag
                    (sts_state == 'h1)  ? 32'h5000_0000       : // sts_flag
                    (sts_state == 'h2)  ? user0 : // app0
                    (sts_state == 'h3)  ? user0 : // app0 
                    (sts_state == 'h4)  ? user1 : // app1
                    (sts_state == 'h5)  ? user1 : // app1
                    (sts_state == 'h6)  ? user2 : // app2
                    (sts_state == 'h7)  ? user2 : // app2 
                    (sts_state == 'h8)  ? user3 : // app3
                    (sts_state == 'h9)  ? user3 : // app3
                    (sts_state == 'ha)  ? user4 : // app4
                    (sts_state == 'hb)  ? user4 : // app4 
                    32'h5000_0000; 
   
   assign s_tvalid = sts_state[0]; 
   assign s_tlast = (sts_state == 'hb) ; 

endmodule  

//=========================================================================================== 
//(* DONT_TOUCH = "yes", keep_hierarchy="yes" *) 
module CTRL_module( clk, reset, 
                    m_tdata, m_tvalid, m_tready, m_tlast, 
                    user0, user1,  user2, user3, user4
                    ) ;

   input        clk; 
   input        reset; 
   input [31:0] m_tdata; 
   input        m_tvalid ;
   input        m_tlast ; 
   output       m_tready; 
   output reg [31:0] user0; 
   output reg [31:0] user1; 
   output reg [31:0] user2; 
   output reg [31:0] user3; 
   output reg [31:0] user4; 
   
   reg [3:0]         cntrl_state ; 
   reg [3:0]         cntrl_flag ; 
   
   assign m_tready = 'b1; //(cntrl_state == 0); 

   always @(posedge clk or posedge reset) begin    
      if (reset) 
        cntrl_state <= 'h0; 
      else if (m_tvalid & m_tlast)
        cntrl_state <= 'h0;
      else if (m_tvalid)
        cntrl_state <= cntrl_state +'h1; 
   end 
   
   always @(posedge clk or posedge reset) begin    
      if (reset) begin 
         cntrl_flag <= 0; 
         user0 <= 0; 
         user1 <= 0; 
         user2 <= 0; 
         user3 <= 0; 
         user4 <= 0; 
      end 
      else if (m_tvalid) begin 
         if (cntrl_state=='h0) 
           cntrl_flag <= m_tdata;
         else if (cntrl_state=='h1) 
           user0 <= m_tdata;
         else if (cntrl_state=='h2) 
           user1 <= m_tdata;
         else if (cntrl_state=='h3) 
           user2 <= m_tdata;       
         else if (cntrl_state=='h4) 
           user3 <= m_tdata;
         else if (cntrl_state=='h5) 
           user4 <= m_tdata;              
      end
   end 

endmodule 
`endif 
