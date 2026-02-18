`timescale 1 ns / 1 ps
//------------------------------------------------------------------------------
`include  "vector.vh"

module io_table
  ( 
    delay_clk,  vec_clk, vec_clk_90, vec_clk_180,  
    resetn, 
    delay0, delay1, pulse_ctrl_bits,
    error_detected, vec_clk_en, 
    pin_type, error, dout, oen, pin_din, pin_dout, pin_oen ) ; 
   input resetn; 
   input delay_clk; // not used 
   input vec_clk; 
   input vec_clk_90;
   input vec_clk_180;
   input vec_clk_en; 
   input error_detected; 
   
   input  wire [511:0] pin_type; // 128*4 = 512
   input  wire [31:0]  delay0; 
   input  wire [31:0]  delay1;
   
   input  wire [2047:0] pulse_ctrl_bits;
   
   output wire [`VECTOR_WIDTH-1:0] error; 
   input  wire [`VECTOR_WIDTH-1:0] dout; 
   input  wire [`VECTOR_WIDTH-1:0] oen;
   
   input  wire [`VECTOR_WIDTH-1:0] pin_din; 
   output wire [`VECTOR_WIDTH-1:0] pin_dout; 
   output wire [`VECTOR_WIDTH-1:0] pin_oen; 
   
   //reg pclk_dout; 

   reg [`VECTOR_WIDTH-1:0]         pin_dout_w; 
   reg [`VECTOR_WIDTH-1:0]         pin_oen_w; 
   reg [`VECTOR_WIDTH-1:0]         error_w; 
   
   (* ASYNC_REG="true" *)reg [`VECTOR_WIDTH-1:0] pin_dout_reg; 
   (* ASYNC_REG="true" *)reg [`VECTOR_WIDTH-1:0] pin_din_reg; 
   (* ASYNC_REG="true" *)reg [`VECTOR_WIDTH-1:0] pin_oen_reg;
   (* ASYNC_REG="true" *)reg [`VECTOR_WIDTH-1:0] error_reg; 
   
   //------------ vec_clk delay lines ------------------
   (* ASYNC_REG="true" *)reg [255:0] delay_line0;
   (* ASYNC_REG="true" *)reg [255:0] delay_line1; 
   (* ASYNC_REG="true" *)reg vec_clk_en_reg ; 
   
   (* mark_debug="false" *) reg delay0_sig;// = delay_line0[delay0[7:0]]; 
   (* mark_debug="false" *) reg delay1_sig;// = delay_line1[delay1[7:0]] ;

   reg [7:0]                       vec_clk_cnt;
   reg                             vec_clk_1d;
   
   always @(posedge delay_clk)
     if(!resetn) begin
        vec_clk_cnt <= 0;
        vec_clk_1d <= 0;
     end else begin
        vec_clk_1d <= vec_clk;
        if(!vec_clk_1d && vec_clk) begin
           vec_clk_cnt <= 0;
        end else begin
           vec_clk_cnt <= vec_clk_cnt + 1;
        end
     end
   
   always @(posedge delay_clk) begin
      //        if (~resetn) begin 
      //            delay_line0 <= 'h0;
      //            delay_line1 <= 'h0; 
      //           delay0_sig <= 0; 
      //            delay1_sig <= 0; 
      //            vec_clk_en_reg <= 0; 
      //        end 
      //        else begin 
      delay_line0 <= {delay_line0[254:0], vec_clk };
      delay_line1 <= {delay_line1[254:0], vec_clk };
      delay0_sig <= delay_line0[delay0[7:0]] ;
      delay1_sig <= delay_line1[delay1[7:0]] ;
      vec_clk_en_reg <= vec_clk_en; 
      //  pin_oen_reg  <= pin_oen_w;  // to buffer
      //  pin_dout_reg <= pin_dout_w; // to buffer
      //  pin_din_reg  <= pin_din;    // from buffer
      //  error_reg    <= error_w; 
   end    
   
   /*reg [`VECTOR_WIDTH-1:0] dout_swapped;
   always@(*) begin
    //dout_swapped[`VECTOR_WIDTH-1:0] = dout[`VECTOR_WIDTH-1:0];

    dout_swapped[95:0] = dout[95:0];
    
    dout_swapped[105] = dout[104];
    dout_swapped[132] = dout[109];
    dout_swapped[121] = dout[120];
    dout_swapped[135] = dout[100];
    dout_swapped[133] = dout[98];
    dout_swapped[130] = dout[96];
    dout_swapped[134] = dout[99];
    dout_swapped[124] = dout[123];
    dout_swapped[106] = dout[105];
    dout_swapped[129] = dout[97];
    dout_swapped[123] = dout[121]; //
    dout_swapped[122] = dout[122]; //
    dout_swapped[136] = dout[101];
    dout_swapped[143] = dout[110];
   end*/
                          
   genvar i; 
   generate begin
      for(i=0;i<128;i=i+1) begin : PIN_TYPE_CTRL
         single_pin single_pin_i
            (
             .delay_clk       (delay_clk),
             .resetn          (resetn),
             .type_shift      (pin_type[(i+1)*4-1:i*4]),
             .pulse_ctrl_bits (pulse_ctrl_bits[(i+1)*16-1:i*16]),
             .vec_clk_cnt     (vec_clk_cnt),
             .dout            (dout[i]),
             .oen             (oen[i]),
             .pin_din         (pin_din[i]),
             .pin_oen_wr      (pin_oen[i]),
             .pin_dout_wr     (pin_dout[i]),
             .error_wr        (error[i]) 
             );
      end // block: PIN_TYPE_CTRL
   end
   endgenerate   
   
endmodule 


module single_pin
  (
   input        delay_clk,
   input        resetn,
   input [3:0]  type_shift,
   input [15:0] pulse_ctrl_bits,
   input [7:0]  vec_clk_cnt,
   input        dout,
   input        oen,
   input        pin_din,
   output reg   pin_oen_wr,
   output reg   pin_dout_wr,
   output reg   error_wr
   );

   localparam   X = 1;
   
   reg [X:0]    pin_oen_w;
   reg [X:0]    pin_dout_w;
   reg [X:0]    error_w;
   
   always @(posedge delay_clk)
     begin
        {error_wr, error_w[X:1]} <= error_w[X:0];
        {pin_dout_wr, pin_dout_w[X:1]} <= pin_dout_w[X:0];
        {pin_oen_wr, pin_oen_w[X:1]} <= pin_oen_w[X:0];
     end
   
   always @(posedge delay_clk)
     if(!resetn) begin
        /*AUTORESET*/
        // Beginning of autoreset for uninitialized flops
        error_w[0] <= 1'h0;
        pin_dout_w[0] <= 1'h0;
        pin_oen_w[0] <= 1'h0;
        // End of automatics
     end else begin
        if (type_shift == `INPUT_PIN) begin  // {X,X, H,L} 
           pin_oen_w[0]  <= 1'b1;       // always input {L,H,X,X}  
           pin_dout_w[0] <= dout; // not used 
           error_w[0] <=  (oen)&(pin_din ^ dout); // {1,0 => Z 
        end 
        
        else if (type_shift == `OUTPUT_PIN) begin // {0,1,Z,Z}   
           pin_oen_w[0]  <= oen  ; // 0=>driver on; 1=>z state output  
           pin_dout_w[0] <= dout ; // 
           error_w[0] <=  1'b0 ; //never causes an error
        end 
        
        else if (type_shift == `OPEN_C_PIN) begin // {0,Z,H,L}
           pin_oen_w[0]  <= (dout | oen); // driver is on (pin_oen_reg =0) for 0 state (oen=0, dout=0)
           pin_dout_w[0] <= 1'b0 ; // always 0  
           error_w[0]    <=  (oen)&(dout ^ pin_din); // {H,L} are checked
        end 
        
        else if (type_shift == `PULSE_PIN) begin  
           //pin_oen_w[0]  = 1'b0;//  always output 
           //pin_dout_w[0] = delay1_sig & dout & vec_clk_en_reg; //vec_clk_90 & dout & vec_clk_en ; // vec_clk_90 = (phase_shift(vec_clk, 90) & vec_clk_en)
           case({dout, oen})
             2'b00: begin
                pin_oen_w[0]  <= 1'b0;
                pin_dout_w[0] <= 1'b0;
             end
             2'b10: begin
                pin_oen_w[0]  <= 1'b0;
                pin_dout_w[0] <= 1'b1;
             end
             2'b01: begin
                pin_oen_w[0]  <= 1'b0;
                //pin_dout_w[0] = delay1_sig & vec_clk_en_reg;
                if(pulse_ctrl_bits[15:8] == vec_clk_cnt)
                  pin_dout_w[0] <= 1'b1;
                if(pulse_ctrl_bits[7:0] == vec_clk_cnt)
                  pin_dout_w[0] <= 1'b0;                 
             end
             default: begin
                pin_oen_w[0]  <= 1'b1;
                pin_dout_w[0] <= 0;
             end
           endcase
           error_w[0] <=  1'b0;
        end 
        
        else if (type_shift == `NPULSE_PIN) begin  // 
           //pin_oen_w[0]  = 1'b0;//  
           //pin_dout_w[0] = ~(delay1_sig & dout & vec_clk_en_reg) ;//~(vec_clk_90 & dout & vec_clk_en); // inverse of PULSE_PIN
           case({dout, oen})
             2'b00: begin
                pin_oen_w[0]  <= 1'b0;
                pin_dout_w[0] <= 1'b0;
             end
             2'b10: begin
                pin_oen_w[0]  <= 1'b0;
                pin_dout_w[0] <= 1'b1;
             end
             2'b01: begin
                pin_oen_w[0]  <= 1'b0;
                //pin_dout_w[0] <= ~(delay1_sig & vec_clk_en_reg);
                if(pulse_ctrl_bits[15:8] == vec_clk_cnt)
                  pin_dout_w[0] <= 1'b0;
                if(pulse_ctrl_bits[7:0] == vec_clk_cnt)
                  pin_dout_w[0] <= 1'b1;  
             end
             default: begin
                pin_oen_w[0]  <= 1'b1;
                pin_dout_w[0] <= 0;
             end
           endcase
           error_w[0] <=  1'b0;
        end // if (type_shift == `NPULSE_PIN)
        
`ifdef CLK_PIN_ENABLE        
        else if (type_shift == `VEC_CLK_PIN) begin  
           pin_oen_w[0]  <= 1'b0; // always output 
           //pin_dout_w[0] <= delay0_sig & vec_clk_en_reg; //vec_clk_180 ;  // vec_clk_180 = (phase_shift(vec_clk, 180) & vec_clk_en)
           if(pulse_ctrl_bits[15:8] == vec_clk_cnt)
             pin_dout_w[0] <= 1'b1;
           if(pulse_ctrl_bits[7:0] == vec_clk_cnt)
             pin_dout_w[0] <= 1'b0;
           error_w[0]    <= 1'b0; // no error
        end 
`endif //  `ifdef CLK_PIN_ENABLE
        
        // these cause serious timing problems                 
        //            else if (type_shift == `ERROR_TRIG) begin  
        //                pin_oen_reg  = 1'b0; // always output 
        //                pin_dout_reg = error_detected;  
        //                error_reg    = 1'b0; // no error (oen)&(din ^ dout); // 1,0 => Z 
        //                end
        
        //            else if (type_shift == `VEC_CLK_EN_PIN) begin  
        //                pin_oen_reg  = 1'b0; // always output 
        //                pin_dout_reg = vec_clk_en; 
        //                error_reg    = 1'b0; // no error 
        //                end            
        
        else begin // BIDI PIN 
           pin_oen_w[0]  <= oen; // 0,1, L,H = (~oen,~dout: ~oen,dout: oen,~dout, oen,dout)
           pin_dout_w[0] <= dout; 
           error_w[0] <=  (oen)&(pin_din ^ dout); // compare H,L
        end
     end

endmodule // single_pin
