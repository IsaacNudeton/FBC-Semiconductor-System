create_interface XADC
set_property INTERFACE XADC [get_ports { Vaux_v_n[15] Vaux_v_n[14] Vaux_v_n[13] Vaux_v_n[12] Vaux_v_n[11] Vaux_v_n[10] Vaux_v_n[9] Vaux_v_n[8] Vaux_v_n[7] Vaux_v_n[6] Vaux_v_n[5] Vaux_v_n[4] Vaux_v_n[3] Vaux_v_n[2] Vaux_v_n[1] Vaux_v_n[0] Vaux_v_p[15] Vaux_v_p[14] Vaux_v_p[13] Vaux_v_p[12] Vaux_v_p[11] Vaux_v_p[10] Vaux_v_p[9] Vaux_v_p[8] Vaux_v_p[7] Vaux_v_p[6] Vaux_v_p[5] Vaux_v_p[4] Vaux_v_p[3] Vaux_v_p[2] Vaux_v_p[1] Vaux_v_p[0] Vp_Vn_v_n Vp_Vn_v_p }]

#set_property IOSTANDARD LVDS_25 [get_ports vec_clk_*]
#set_property IOSTANDARD LVDS_25 [get_ports clk_out*]
set_property IOSTANDARD LVCMOS25 [get_ports gpio*]
set_property IOSTANDARD LVCMOS25 [get_ports Vaux_v_n*]
set_property IOSTANDARD LVCMOS25 [get_ports Vaux_v_p*]

# gpio[97, 101, 105, 110, 121, 122, 123] = MDIO[0:6]drive strength 4 milAmp
# 105 = MDIO1
# 121 = MDIO4
# 97  = mdio2
# 123 = MDIO0
# 122 = MDIO3
# 101 = MDIO5
# 110 = MDIO6

# TDO = gpio[92]  drive strength 4Ma (

#set_property PULLUP true [get_ports {gpio[11]}]
#set_property PULLUP true [get_ports vec_clk_n]

#set_property PACKAGE_PIN Y8 [get_ports vec_clk_n];
#set_property PACKAGE_PIN Y9 [get_ports vec_clk_p];

#set_property PACKAGE_PIN D18 [get_ports {clk_out[0]}]
#set_property PACKAGE_PIN Y18 [get_ports {clk_out[1]}]
#set_property PACKAGE_PIN L18 [get_ports {clk_out[2]}]
#set_property PACKAGE_PIN B19 [get_ports {clk_out[3]}]

#set_property PACKAGE_PIN B19 [get_ports {clk_out_p[3]}]

#set_property PACKAGE_PIN Y8 [get_ports vec_clk_n]
#set_property PACKAGE_PIN B20 [get_ports {clk_out_n[3]}]

#set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[3]}]
#set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[2]}]
#set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[1]}]
#set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[0]}]

#set_property IOSTANDARD LVCMOS25 [get_ports {clk_out_n[3]}]
#set_property IOSTANDARD LVCMOS25 [get_ports {clk_out_n[2]}]
#set_property IOSTANDARD LVCMOS25 [get_ports {clk_out_n[1]}]
#set_property IOSTANDARD LVCMOS25 [get_ports {clk_out_n[0]}]
##set_property IOSTANDARD LVCMOS25 [get_ports vec_clk_n]
##set_property IOSTANDARD LVCMOS25 [get_ports vec_clk_p]

set_property PACKAGE_PIN L18 [get_ports {clk_out_p[3]}]
set_property PACKAGE_PIN Y18 [get_ports {clk_out_p[2]}]
set_property PACKAGE_PIN Y9 [get_ports {clk_out_p[1]}]
set_property PACKAGE_PIN D18 [get_ports {clk_out_p[0]}]

#set_property PACKAGE_PIN B19 [get_ports vec_clk_p]
#set_property PACKAGE_PIN B20 [get_ports vec_clk_n]


#set_property PACKAGE_PIN D18 [get_ports {clk_out_p[0]}]
#set_property PACKAGE_PIN Y9 [get_ports {clk_out_p[1]}]
#set_property PACKAGE_PIN Y18 [get_ports {clk_out_p[2]}]
#set_property PACKAGE_PIN L18 [get_ports {clk_out_p[3]}]

#set_property PACKAGE_PIN C19 [get_ports {clk_out_n[0]}]
#set_property PACKAGE_PIN Y8 [get_ports {clk_out_n[1]}]
#set_property PACKAGE_PIN AA18 [get_ports {clk_out_n[2]}]
#set_property PACKAGE_PIN L19 [get_ports {clk_out_n[3]}]



#set_property PACKAGE_PIN K16 [get_ports pwm_PUDC]
#set_property IOSTANDARD LVCMOS33 [get_ports pwm_PUDC]


#set_property PACKAGE_PIN K16 [get_ports {pwm_PUDC}] ; # PUDC_B/gpio[157]

set_property PACKAGE_PIN D15 [get_ports {Vaux_v_n[1]}]

set_property DRIVE 4 [get_ports {gpio[159]}]
set_property DRIVE 4 [get_ports {gpio[158]}]
set_property DRIVE 4 [get_ports {gpio[157]}]
set_property DRIVE 4 [get_ports {gpio[156]}]
set_property DRIVE 4 [get_ports {gpio[155]}]
set_property DRIVE 4 [get_ports {gpio[154]}]
set_property DRIVE 4 [get_ports {gpio[153]}]
set_property DRIVE 4 [get_ports {gpio[152]}]
set_property DRIVE 4 [get_ports {gpio[151]}]
set_property DRIVE 4 [get_ports {gpio[150]}]
set_property DRIVE 4 [get_ports {gpio[149]}]
set_property DRIVE 4 [get_ports {gpio[148]}]
set_property DRIVE 4 [get_ports {gpio[147]}]
set_property DRIVE 4 [get_ports {gpio[146]}]
set_property DRIVE 4 [get_ports {gpio[145]}]
set_property DRIVE 4 [get_ports {gpio[144]}]
set_property DRIVE 4 [get_ports {gpio[143]}]
set_property DRIVE 4 [get_ports {gpio[142]}]
set_property DRIVE 4 [get_ports {gpio[141]}]
set_property DRIVE 4 [get_ports {gpio[140]}]
set_property DRIVE 4 [get_ports {gpio[139]}]
set_property DRIVE 4 [get_ports {gpio[138]}]
set_property DRIVE 4 [get_ports {gpio[137]}]
set_property DRIVE 4 [get_ports {gpio[136]}]
set_property DRIVE 4 [get_ports {gpio[135]}]
set_property DRIVE 4 [get_ports {gpio[134]}]
set_property DRIVE 4 [get_ports {gpio[133]}]
set_property DRIVE 4 [get_ports {gpio[132]}]
set_property DRIVE 4 [get_ports {gpio[131]}]
set_property DRIVE 4 [get_ports {gpio[130]}]
set_property DRIVE 4 [get_ports {gpio[129]}]
set_property DRIVE 4 [get_ports {gpio[128]}]
set_property DRIVE 4 [get_ports {gpio[127]}]
set_property DRIVE 4 [get_ports {gpio[126]}]
set_property DRIVE 4 [get_ports {gpio[125]}]
set_property DRIVE 4 [get_ports {gpio[124]}]
set_property DRIVE 4 [get_ports {gpio[123]}]
set_property DRIVE 4 [get_ports {gpio[122]}]
set_property DRIVE 4 [get_ports {gpio[121]}]
set_property DRIVE 4 [get_ports {gpio[120]}]
set_property DRIVE 4 [get_ports {gpio[119]}]
set_property DRIVE 4 [get_ports {gpio[118]}]
set_property DRIVE 4 [get_ports {gpio[117]}]
set_property DRIVE 4 [get_ports {gpio[116]}]
set_property DRIVE 4 [get_ports {gpio[115]}]
set_property DRIVE 4 [get_ports {gpio[114]}]
set_property DRIVE 4 [get_ports {gpio[113]}]
set_property DRIVE 4 [get_ports {gpio[112]}]
set_property DRIVE 4 [get_ports {gpio[111]}]
set_property DRIVE 4 [get_ports {gpio[110]}]
set_property DRIVE 4 [get_ports {gpio[109]}]
set_property DRIVE 4 [get_ports {gpio[108]}]
set_property DRIVE 4 [get_ports {gpio[107]}]
set_property DRIVE 4 [get_ports {gpio[106]}]
set_property DRIVE 4 [get_ports {gpio[105]}]
set_property DRIVE 4 [get_ports {gpio[104]}]
set_property DRIVE 4 [get_ports {gpio[103]}]
set_property DRIVE 4 [get_ports {gpio[102]}]
set_property DRIVE 4 [get_ports {gpio[101]}]
set_property DRIVE 4 [get_ports {gpio[100]}]
set_property DRIVE 4 [get_ports {gpio[99]}]
set_property DRIVE 4 [get_ports {gpio[98]}]
set_property DRIVE 4 [get_ports {gpio[97]}]
set_property DRIVE 4 [get_ports {gpio[96]}]
set_property DRIVE 4 [get_ports {gpio[95]}]
set_property DRIVE 4 [get_ports {gpio[94]}]
set_property DRIVE 4 [get_ports {gpio[93]}]
set_property DRIVE 4 [get_ports {gpio[92]}]
set_property DRIVE 4 [get_ports {gpio[91]}]
set_property DRIVE 4 [get_ports {gpio[90]}]
set_property DRIVE 4 [get_ports {gpio[89]}]
set_property DRIVE 4 [get_ports {gpio[88]}]
set_property DRIVE 4 [get_ports {gpio[87]}]
set_property DRIVE 4 [get_ports {gpio[86]}]
set_property DRIVE 4 [get_ports {gpio[85]}]
set_property DRIVE 4 [get_ports {gpio[84]}]
set_property DRIVE 4 [get_ports {gpio[83]}]
set_property DRIVE 4 [get_ports {gpio[82]}]
set_property DRIVE 4 [get_ports {gpio[81]}]
set_property DRIVE 4 [get_ports {gpio[80]}]
set_property DRIVE 4 [get_ports {gpio[79]}]
set_property DRIVE 4 [get_ports {gpio[78]}]
set_property DRIVE 4 [get_ports {gpio[77]}]
set_property DRIVE 4 [get_ports {gpio[76]}]
set_property DRIVE 4 [get_ports {gpio[75]}]
set_property DRIVE 4 [get_ports {gpio[74]}]
set_property DRIVE 4 [get_ports {gpio[73]}]
set_property DRIVE 4 [get_ports {gpio[72]}]
set_property DRIVE 4 [get_ports {gpio[71]}]
set_property DRIVE 4 [get_ports {gpio[70]}]
set_property DRIVE 4 [get_ports {gpio[69]}]
set_property DRIVE 4 [get_ports {gpio[68]}]
set_property DRIVE 4 [get_ports {gpio[67]}]
set_property DRIVE 4 [get_ports {gpio[66]}]
set_property DRIVE 4 [get_ports {gpio[65]}]
set_property DRIVE 4 [get_ports {gpio[64]}]
set_property DRIVE 4 [get_ports {gpio[63]}]
set_property DRIVE 4 [get_ports {gpio[62]}]
set_property DRIVE 4 [get_ports {gpio[61]}]
set_property DRIVE 4 [get_ports {gpio[60]}]
set_property DRIVE 4 [get_ports {gpio[59]}]
set_property DRIVE 4 [get_ports {gpio[58]}]
set_property DRIVE 4 [get_ports {gpio[57]}]
set_property DRIVE 4 [get_ports {gpio[56]}]
set_property DRIVE 4 [get_ports {gpio[55]}]
set_property DRIVE 4 [get_ports {gpio[54]}]
set_property DRIVE 4 [get_ports {gpio[53]}]
set_property DRIVE 4 [get_ports {gpio[52]}]
set_property DRIVE 4 [get_ports {gpio[51]}]
set_property DRIVE 4 [get_ports {gpio[50]}]
set_property DRIVE 4 [get_ports {gpio[49]}]
set_property DRIVE 4 [get_ports {gpio[48]}]
set_property DRIVE 4 [get_ports {gpio[47]}]
set_property DRIVE 4 [get_ports {gpio[46]}]
set_property DRIVE 4 [get_ports {gpio[45]}]
set_property DRIVE 4 [get_ports {gpio[44]}]
set_property DRIVE 4 [get_ports {gpio[43]}]
set_property DRIVE 4 [get_ports {gpio[42]}]
set_property DRIVE 4 [get_ports {gpio[41]}]
set_property DRIVE 4 [get_ports {gpio[40]}]
set_property DRIVE 4 [get_ports {gpio[39]}]
set_property DRIVE 4 [get_ports {gpio[38]}]
set_property DRIVE 4 [get_ports {gpio[37]}]
set_property DRIVE 4 [get_ports {gpio[36]}]
set_property DRIVE 4 [get_ports {gpio[35]}]
set_property DRIVE 4 [get_ports {gpio[34]}]
set_property DRIVE 4 [get_ports {gpio[33]}]
set_property DRIVE 4 [get_ports {gpio[32]}]
set_property DRIVE 4 [get_ports {gpio[31]}]
set_property DRIVE 4 [get_ports {gpio[30]}]
set_property DRIVE 4 [get_ports {gpio[29]}]
set_property DRIVE 4 [get_ports {gpio[28]}]
set_property DRIVE 4 [get_ports {gpio[27]}]
set_property DRIVE 4 [get_ports {gpio[26]}]
set_property DRIVE 4 [get_ports {gpio[25]}]
set_property DRIVE 4 [get_ports {gpio[24]}]
set_property DRIVE 4 [get_ports {gpio[23]}]
set_property DRIVE 4 [get_ports {gpio[22]}]
set_property DRIVE 4 [get_ports {gpio[21]}]
set_property DRIVE 4 [get_ports {gpio[20]}]
set_property DRIVE 4 [get_ports {gpio[19]}]
set_property DRIVE 4 [get_ports {gpio[18]}]
set_property DRIVE 4 [get_ports {gpio[17]}]
set_property DRIVE 8 [get_ports {gpio[16]}]
set_property DRIVE 8 [get_ports {gpio[15]}]
set_property DRIVE 8 [get_ports {gpio[14]}]
set_property DRIVE 8 [get_ports {gpio[13]}]
set_property DRIVE 8 [get_ports {gpio[12]}]
set_property DRIVE 8 [get_ports {gpio[11]}]
set_property DRIVE 8 [get_ports {gpio[10]}]
set_property DRIVE 8 [get_ports {gpio[9]}]
set_property DRIVE 8 [get_ports {gpio[8]}]
set_property DRIVE 8 [get_ports {gpio[7]}]
set_property DRIVE 8 [get_ports {gpio[6]}]
set_property DRIVE 8 [get_ports {gpio[5]}]
set_property DRIVE 8 [get_ports {gpio[4]}]
set_property DRIVE 8 [get_ports {gpio[3]}]
set_property DRIVE 8 [get_ports {gpio[2]}]
set_property DRIVE 8 [get_ports {gpio[1]}]
set_property DRIVE 8 [get_ports {gpio[0]}]

#create_generated_clock -name vect_out_clk  -divide_by 1 -source [get_pins io_table/genblk1.PIN_TYPE_CTRL[123].single_pin_i/pin_oen_wr_reg/C] [get_ports {gpio[123]}]
#set_output_delay -max 1.5 -clock [get_clocks vect_out_clk] [get_ports {gpio[*]}]
#set_output_delay -min -0.5 -clock [get_clocks vect_out_clk] [get_ports {gpio[*]}]

#set_input_delay -clock [get_clocks clk_out1_design_1_fclk1_pll_0_0] -max 2.000 [get_ports {gpio[*]}]
#set_input_delay -clock [get_clocks clk_out1_design_1_fclk1_pll_0_0] -min 1.000 [get_ports {gpio[*]}]

set_false_path -from [get_pins {design_1_i/pll_en_sel/U0/gpio_core_1/Not_Dual.gpio_Data_Out_reg[*]/C}] -to [get_pins design_1_i/vec_clk_pll/inst/CLK_CORE_DRP_I/clk_inst/clkout*_buf/CE0]
set_false_path -from [get_ports {gpio[*]}] -to [get_pins design_1_i/vec_clk_pll/inst/CLK_CORE_DRP_I/clk_inst/clkout*_buf/CE*]
#set_false_path -from [get_pins {axi_io_table_inst/delay0_reg[*]/C}] -to [get_pins io_table/delay0_sig_reg/D]
#set_false_path -from [get_pins {axi_io_table_inst/delay1_reg[*]/C}] -to [get_pins {io_table/delay1_sig_reg/D io_table/delay1_sig_reg_replica/D}]
set_false_path -from [get_pins {axi_io_table_inst/control_bits_reg[*]/C}] -to [get_ports {gpio[*]}]

set_false_path -through [get_pins {axi_io_table_inst/control_bits_reg[*]/C}]
set_false_path -through [get_pins {io_table/genblk*.PIN_TYPE_CTRL[*].single_pin_i/error_wr_reg/C}]

set_multicycle_path -setup -end -from [get_pins repeat_counter/s_cycle_reg/C] -to [get_pins io_table/vec_clk_en_reg_reg/D] 2
set_multicycle_path -hold -end -from [get_pins repeat_counter/s_cycle_reg/C] -to [get_pins io_table/vec_clk_en_reg_reg/D] 1
set_multicycle_path -hold -start -from [get_pins design_1_i/vec_clk_pll/inst/CLK_CORE_DRP_I/clk_inst/mmcm_adv_inst/CLKOUT0] -to [get_pins {io_table/delay_line*_reg[0]/D}] 1

set_max_delay -datapath_only -from [get_ports {gpio[*]}] -to [get_pins {axi_freq_counter_inst/freq_counters[*].freq_counter_inst/sig_reg[0]/D}] 10.000
set_max_delay -datapath_only -from [get_ports {gpio[*]}] -to [get_pins {axi_freq_counter_inst/freq_counters[*].freq_counter_inst/trig_reg[0]/D}] 10.000
set_max_delay -datapath_only -from [get_ports {gpio[*]}] -to [get_pins {ERROR_counter/error_count_reg[*]/CE}] 20.000
set_max_delay -datapath_only -from [get_ports {gpio[*]}] -to [get_pins ERROR_counter/first_error_detected_reg/D] 20.000
set_max_delay -datapath_only -from [get_pins {repeat_counter/s_tdata_reg[*]/C}] -to [get_ports {gpio[*]}] 20.000
set_max_delay -datapath_only -from [get_pins {design_1_i/extra_gpio/U0/gpio_core_1/Not_Dual.gpio_*_reg[*]/C}] -to [get_ports {gpio[*]}] 20.000

set_max_delay -datapath_only -from [get_pins {io_table/delay_line*_reg[*]/C}] -to [get_ports {gpio[*]}] 5.000
set_min_delay -from [get_pins {io_table/delay_line*_reg[*]/C}] -to [get_ports {gpio[*]}] 0.000
set_max_delay -datapath_only -from [get_pins io_table/vec_clk_en_reg*/C] -to [get_ports {gpio[*]}] 5.000
set_min_delay -from [get_pins io_table/vec_clk_en_reg*/C] -to [get_ports {gpio[*]}] 0.000

#set_max_delay -datapath_only -from [get_pins io_table/delay.*_sig.*/C -regexp -hierarchical] -to [get_ports {gpio[*]}] 5.000

#set_property OFFCHIP_TERM NONE [get_ports {clk_out_n[0]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_n[1]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_n[2]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_n[3]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_p[0]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_p[1]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_p[2]}]
#set_property OFFCHIP_TERM NONE [get_ports {clk_out_p[3]}]
#set_property DIFF_TERM TRUE [get_ports {clk_out_p[3]}]
