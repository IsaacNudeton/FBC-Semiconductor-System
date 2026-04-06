#==============================================================================
# FBC Semiconductor System - Zynq 7020 Sonoma Constraints
#==============================================================================
# Target: XC7Z020-1CLG484C (HPBIController PCB, Rev A-D)
#
# Pin assignments derived from:
#   reference/kzhang_v2_2016/gpio_old_board.xdc   (original Vivado 2015.4 design)
#   reference/kzhang_v2_2016/broadcom_v21d.xdc    (drive strength + timing)
#
# Verified against:
#   reference/hpbicontroller-rev1/                 (Altium schematics, BOM)
#   reference/sonoma_docs/04_VERIFIED_FROM_DEVICE_FILES.md  (ELF disassembly)
#   reference/sonoma_docs/01_HARDWARE_REFERENCE.md (component inventory)
#
# AXI Address Map (differs between Sonoma and FBC RTL):
#   Sonoma (kzhang):  0x43C00000 (design), 0x404E0000 (verified from ELF)
#   FBC (system_top): 0x40040000 (our address decode)
#
# Pin Architecture (160 total):
#   gpio[0:47]    - Bank 13 (48 pins, through BIM to DUT)
#   gpio[48:95]   - Bank 33 (48 pins, through BIM to DUT)
#   gpio[96:127]  - Bank 34 (32 pins, through BIM to DUT)
#   gpio[128:159] - Direct FPGA pins (32 pins, no BIM, low latency)
#
# Special Pin Functions (from reference comments):
#   gpio[92]      - TDO (JTAG)
#   gpio[97,101,105,110,121,122,123] - MDIO[0:6] control pins
#   gpio[123]     - MDIO0 (often used as vec_clk output)
#
#==============================================================================

#==============================================================================
# Bank 13 - gpio[0:47] - BIM Pins
#==============================================================================
set_property PACKAGE_PIN R7   [get_ports {gpio[0]}]
set_property PACKAGE_PIN V9   [get_ports {gpio[1]}]
set_property PACKAGE_PIN V10  [get_ports {gpio[2]}]
set_property PACKAGE_PIN W8   [get_ports {gpio[3]}]
set_property PACKAGE_PIN V8   [get_ports {gpio[4]}]
set_property PACKAGE_PIN W10  [get_ports {gpio[5]}]
set_property PACKAGE_PIN W11  [get_ports {gpio[6]}]
set_property PACKAGE_PIN W12  [get_ports {gpio[7]}]
set_property PACKAGE_PIN V12  [get_ports {gpio[8]}]
set_property PACKAGE_PIN U11  [get_ports {gpio[9]}]
set_property PACKAGE_PIN U12  [get_ports {gpio[10]}]
set_property PACKAGE_PIN U9   [get_ports {gpio[11]}]
set_property PACKAGE_PIN U10  [get_ports {gpio[12]}]
set_property PACKAGE_PIN AB12 [get_ports {gpio[13]}]
set_property PACKAGE_PIN AA12 [get_ports {gpio[14]}]
set_property PACKAGE_PIN AB11 [get_ports {gpio[15]}]
set_property PACKAGE_PIN AA11 [get_ports {gpio[16]}]
set_property PACKAGE_PIN AB9  [get_ports {gpio[17]}]
set_property PACKAGE_PIN AB10 [get_ports {gpio[18]}]
set_property PACKAGE_PIN Y10  [get_ports {gpio[19]}]
set_property PACKAGE_PIN Y11  [get_ports {gpio[20]}]
set_property PACKAGE_PIN AA8  [get_ports {gpio[21]}]
set_property PACKAGE_PIN AA9  [get_ports {gpio[22]}]
set_property PACKAGE_PIN Y5   [get_ports {gpio[23]}]
set_property PACKAGE_PIN Y6   [get_ports {gpio[24]}]
set_property PACKAGE_PIN AA6  [get_ports {gpio[25]}]
set_property PACKAGE_PIN AA7  [get_ports {gpio[26]}]
set_property PACKAGE_PIN AB1  [get_ports {gpio[27]}]
set_property PACKAGE_PIN AB2  [get_ports {gpio[28]}]
set_property PACKAGE_PIN AB4  [get_ports {gpio[29]}]
set_property PACKAGE_PIN AB5  [get_ports {gpio[30]}]
set_property PACKAGE_PIN AB6  [get_ports {gpio[31]}]
set_property PACKAGE_PIN AB7  [get_ports {gpio[32]}]
set_property PACKAGE_PIN AA4  [get_ports {gpio[33]}]
set_property PACKAGE_PIN Y4   [get_ports {gpio[34]}]
set_property PACKAGE_PIN R6   [get_ports {gpio[35]}]
set_property PACKAGE_PIN T6   [get_ports {gpio[36]}]
set_property PACKAGE_PIN U4   [get_ports {gpio[37]}]
set_property PACKAGE_PIN T4   [get_ports {gpio[38]}]
set_property PACKAGE_PIN V4   [get_ports {gpio[39]}]
set_property PACKAGE_PIN V5   [get_ports {gpio[40]}]
set_property PACKAGE_PIN U5   [get_ports {gpio[41]}]
set_property PACKAGE_PIN U6   [get_ports {gpio[42]}]
set_property PACKAGE_PIN W7   [get_ports {gpio[43]}]
set_property PACKAGE_PIN V7   [get_ports {gpio[44]}]
set_property PACKAGE_PIN W5   [get_ports {gpio[45]}]
set_property PACKAGE_PIN W6   [get_ports {gpio[46]}]
set_property PACKAGE_PIN U7   [get_ports {gpio[47]}]

#==============================================================================
# Bank 33 - gpio[48:95] - BIM Pins
#==============================================================================
set_property PACKAGE_PIN U19  [get_ports {gpio[48]}]
set_property PACKAGE_PIN U21  [get_ports {gpio[49]}]
set_property PACKAGE_PIN T21  [get_ports {gpio[50]}]
set_property PACKAGE_PIN U22  [get_ports {gpio[51]}]
set_property PACKAGE_PIN T22  [get_ports {gpio[52]}]
set_property PACKAGE_PIN W22  [get_ports {gpio[53]}]
set_property PACKAGE_PIN V22  [get_ports {gpio[54]}]
set_property PACKAGE_PIN W21  [get_ports {gpio[55]}]
set_property PACKAGE_PIN W20  [get_ports {gpio[56]}]
set_property PACKAGE_PIN V20  [get_ports {gpio[57]}]
set_property PACKAGE_PIN U20  [get_ports {gpio[58]}]
set_property PACKAGE_PIN V19  [get_ports {gpio[59]}]
set_property PACKAGE_PIN V18  [get_ports {gpio[60]}]
set_property PACKAGE_PIN AB22 [get_ports {gpio[61]}]
set_property PACKAGE_PIN AA22 [get_ports {gpio[62]}]
set_property PACKAGE_PIN AB21 [get_ports {gpio[63]}]
set_property PACKAGE_PIN AA21 [get_ports {gpio[64]}]
set_property PACKAGE_PIN Y21  [get_ports {gpio[65]}]
set_property PACKAGE_PIN Y20  [get_ports {gpio[66]}]
set_property PACKAGE_PIN AB20 [get_ports {gpio[67]}]
set_property PACKAGE_PIN AB19 [get_ports {gpio[68]}]
set_property PACKAGE_PIN AA19 [get_ports {gpio[69]}]
set_property PACKAGE_PIN Y19  [get_ports {gpio[70]}]
set_property PACKAGE_PIN W18  [get_ports {gpio[71]}]
set_property PACKAGE_PIN W17  [get_ports {gpio[72]}]
set_property PACKAGE_PIN Y16  [get_ports {gpio[73]}]
set_property PACKAGE_PIN W16  [get_ports {gpio[74]}]
set_property PACKAGE_PIN U16  [get_ports {gpio[75]}]
set_property PACKAGE_PIN U15  [get_ports {gpio[76]}]
set_property PACKAGE_PIN V17  [get_ports {gpio[77]}]
set_property PACKAGE_PIN U17  [get_ports {gpio[78]}]
set_property PACKAGE_PIN AB17 [get_ports {gpio[79]}]
set_property PACKAGE_PIN AA17 [get_ports {gpio[80]}]
set_property PACKAGE_PIN AB16 [get_ports {gpio[81]}]
set_property PACKAGE_PIN AA16 [get_ports {gpio[82]}]
set_property PACKAGE_PIN V14  [get_ports {gpio[83]}]
set_property PACKAGE_PIN V15  [get_ports {gpio[84]}]
set_property PACKAGE_PIN W13  [get_ports {gpio[85]}]
set_property PACKAGE_PIN V13  [get_ports {gpio[86]}]
set_property PACKAGE_PIN Y15  [get_ports {gpio[87]}]
set_property PACKAGE_PIN W15  [get_ports {gpio[88]}]
set_property PACKAGE_PIN AA14 [get_ports {gpio[89]}]
set_property PACKAGE_PIN Y14  [get_ports {gpio[90]}]
set_property PACKAGE_PIN AA13 [get_ports {gpio[91]}]
set_property PACKAGE_PIN Y13  [get_ports {gpio[92]}]
set_property PACKAGE_PIN AB15 [get_ports {gpio[93]}]
set_property PACKAGE_PIN AB14 [get_ports {gpio[94]}]
set_property PACKAGE_PIN U14  [get_ports {gpio[95]}]

#==============================================================================
# Bank 34 - gpio[96:127] - BIM Pins (new board mapping)
#==============================================================================
set_property PACKAGE_PIN P20  [get_ports {gpio[96]}]
set_property PACKAGE_PIN P21  [get_ports {gpio[97]}]
set_property PACKAGE_PIN P18  [get_ports {gpio[98]}]
set_property PACKAGE_PIN P17  [get_ports {gpio[99]}]
set_property PACKAGE_PIN T17  [get_ports {gpio[100]}]
set_property PACKAGE_PIN T16  [get_ports {gpio[101]}]
set_property PACKAGE_PIN K16  [get_ports {gpio[102]}]
set_property PACKAGE_PIN M17  [get_ports {gpio[103]}]
set_property PACKAGE_PIN N17  [get_ports {gpio[104]}]
set_property PACKAGE_PIN L17  [get_ports {gpio[105]}]
set_property PACKAGE_PIN N18  [get_ports {gpio[106]}]
set_property PACKAGE_PIN M16  [get_ports {gpio[107]}]
set_property PACKAGE_PIN M15  [get_ports {gpio[108]}]
set_property PACKAGE_PIN N15  [get_ports {gpio[109]}]
set_property PACKAGE_PIN R15  [get_ports {gpio[110]}]
set_property PACKAGE_PIN J22  [get_ports {gpio[111]}]
set_property PACKAGE_PIN J21  [get_ports {gpio[112]}]
set_property PACKAGE_PIN K21  [get_ports {gpio[113]}]
set_property PACKAGE_PIN J20  [get_ports {gpio[114]}]
set_property PACKAGE_PIN L22  [get_ports {gpio[115]}]
set_property PACKAGE_PIN L21  [get_ports {gpio[116]}]
set_property PACKAGE_PIN K20  [get_ports {gpio[117]}]
set_property PACKAGE_PIN K19  [get_ports {gpio[118]}]
set_property PACKAGE_PIN M20  [get_ports {gpio[119]}]
set_property PACKAGE_PIN M21  [get_ports {gpio[120]}]
set_property PACKAGE_PIN M19  [get_ports {gpio[121]}]
set_property PACKAGE_PIN N20  [get_ports {gpio[122]}]
set_property PACKAGE_PIN N19  [get_ports {gpio[123]}]
set_property PACKAGE_PIN M22  [get_ports {gpio[124]}]
set_property PACKAGE_PIN P22  [get_ports {gpio[125]}]
set_property PACKAGE_PIN N22  [get_ports {gpio[126]}]
set_property PACKAGE_PIN R21  [get_ports {gpio[127]}]

#==============================================================================
# Direct FPGA Pins - gpio[128:159] - Fast Vector / Triggers
#==============================================================================
# These pins bypass BIM for lower latency (single cycle vs 2-cycle pipeline)
# Use for: scope triggers, external clock, handshake signals
#==============================================================================
set_property PACKAGE_PIN R20  [get_ports {gpio[128]}]
set_property PACKAGE_PIN K15  [get_ports {gpio[129]}]
set_property PACKAGE_PIN H15  [get_ports {gpio[130]}]
set_property PACKAGE_PIN P15  [get_ports {gpio[131]}]
set_property PACKAGE_PIN K18  [get_ports {gpio[132]}]
set_property PACKAGE_PIN J15  [get_ports {gpio[133]}]
set_property PACKAGE_PIN J17  [get_ports {gpio[134]}]
set_property PACKAGE_PIN J16  [get_ports {gpio[135]}]
set_property PACKAGE_PIN L16  [get_ports {gpio[136]}]
set_property PACKAGE_PIN T19  [get_ports {gpio[137]}]
set_property PACKAGE_PIN R19  [get_ports {gpio[138]}]
set_property PACKAGE_PIN T18  [get_ports {gpio[139]}]
set_property PACKAGE_PIN R18  [get_ports {gpio[140]}]
set_property PACKAGE_PIN R16  [get_ports {gpio[141]}]
set_property PACKAGE_PIN P16  [get_ports {gpio[142]}]
set_property PACKAGE_PIN J18  [get_ports {gpio[143]}]
set_property PACKAGE_PIN H17  [get_ports {gpio[144]}]
set_property PACKAGE_PIN G16  [get_ports {gpio[145]}]
set_property PACKAGE_PIN G15  [get_ports {gpio[146]}]
set_property PACKAGE_PIN F17  [get_ports {gpio[147]}]
set_property PACKAGE_PIN G17  [get_ports {gpio[148]}]
set_property PACKAGE_PIN C18  [get_ports {gpio[149]}]
set_property PACKAGE_PIN C17  [get_ports {gpio[150]}]
set_property PACKAGE_PIN C22  [get_ports {gpio[151]}]
set_property PACKAGE_PIN D22  [get_ports {gpio[152]}]
set_property PACKAGE_PIN H20  [get_ports {gpio[153]}]
set_property PACKAGE_PIN H19  [get_ports {gpio[154]}]
set_property PACKAGE_PIN F22  [get_ports {gpio[155]}]
set_property PACKAGE_PIN F21  [get_ports {gpio[156]}]
set_property PACKAGE_PIN H18  [get_ports {gpio[157]}]
set_property PACKAGE_PIN B19  [get_ports {gpio[158]}]
set_property PACKAGE_PIN B20  [get_ports {gpio[159]}]

#==============================================================================
# Clock Outputs (4 differential pairs to DUT)
#==============================================================================
# clk_out[0]: Variable frequency (5/10/25/50/100 MHz) - Pin D18/C19
# clk_out[1]: Fixed 100 MHz CML                       - Pin Y9/Y8
# clk_out[2]: Variable 10-25 MHz                      - Pin Y18/Y17
# clk_out[3]: Fixed 10 MHz differential               - Pin L18/L19
#==============================================================================
set_property PACKAGE_PIN D18  [get_ports {clk_out_p[0]}]
set_property PACKAGE_PIN Y9   [get_ports {clk_out_p[1]}]
set_property PACKAGE_PIN Y18  [get_ports {clk_out_p[2]}]
set_property PACKAGE_PIN L18  [get_ports {clk_out_p[3]}]

set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[*]}]

# LVDS I/O standard for differential clock outputs
set_property IOSTANDARD LVDS_25 [get_ports {clk_out_p[*]}]

#==============================================================================
# I/O Standards
#==============================================================================
set_property IOSTANDARD LVCMOS25 [get_ports {gpio[*]}]

#==============================================================================
# Drive Strength (from reference broadcom_v21d.xdc)
#==============================================================================
# gpio[0:16] - 8mA (high drive for clock/critical signals)
# gpio[17:159] - 4mA (standard)
#==============================================================================
set_property DRIVE 8 [get_ports {gpio[0]}]
set_property DRIVE 8 [get_ports {gpio[1]}]
set_property DRIVE 8 [get_ports {gpio[2]}]
set_property DRIVE 8 [get_ports {gpio[3]}]
set_property DRIVE 8 [get_ports {gpio[4]}]
set_property DRIVE 8 [get_ports {gpio[5]}]
set_property DRIVE 8 [get_ports {gpio[6]}]
set_property DRIVE 8 [get_ports {gpio[7]}]
set_property DRIVE 8 [get_ports {gpio[8]}]
set_property DRIVE 8 [get_ports {gpio[9]}]
set_property DRIVE 8 [get_ports {gpio[10]}]
set_property DRIVE 8 [get_ports {gpio[11]}]
set_property DRIVE 8 [get_ports {gpio[12]}]
set_property DRIVE 8 [get_ports {gpio[13]}]
set_property DRIVE 8 [get_ports {gpio[14]}]
set_property DRIVE 8 [get_ports {gpio[15]}]
set_property DRIVE 8 [get_ports {gpio[16]}]

# All other pins default to 4mA (set explicitly for clarity)
set_property DRIVE 4 [get_ports {gpio[17]}]
set_property DRIVE 4 [get_ports {gpio[18]}]
set_property DRIVE 4 [get_ports {gpio[19]}]
set_property DRIVE 4 [get_ports {gpio[20]}]
# ... (remaining pins get 4mA by default)

#==============================================================================
# Clock MUX Cascade Override — REMOVED (clk_wiz replaces BUFGMUX tree)
#==============================================================================

#==============================================================================
# IOB Register Packing
#==============================================================================
# NOTE: These use old kzhang_v2 hierarchy names (io_table, axi_io_table_inst).
# Our design uses different module names (io_bank, io_config). Uncomment and
# update paths when IOB packing is needed for timing closure.
# set_property IOB true [get_cells {u_fbc_top/u_io_bank/gen_bim_cells[*].u_io_cell/dout_reg}]
# set_property IOB true [get_cells {u_fbc_top/u_io_bank/gen_bim_cells[*].u_io_cell/oen_reg}]

#==============================================================================
# Timing Constraints
#==============================================================================
# TODO: Add proper timing constraints for our io_bank module paths.
# The kzhang_v2 constraints below use old hierarchy names and are disabled.
# set_max_delay -datapath_only -from [get_pins {u_fbc_top/u_io_bank/...}] -to [get_ports {gpio[*]}] 5.000

#==============================================================================
# Clock Domain Crossing Constraints
#==============================================================================
# Architecture has 3 clock domains:
#   1. clk_fpga_0 (100MHz) — AXI bus, firmware registers
#   2. clk_fpga_1 (200MHz) — delay_clk, io_cell pipeline, vec_clk_cnt
#   3. vec_clk (5-100MHz via BUFGMUX) — vector execution timing
#
# MMCM generates 5 clocks (CLKOUT0-4) + 2 phase-shifted (CLKOUT5-6).
# BUFGMUX selects one as vec_clk. Vivado sees ALL MMCM outputs as potential
# sources, creating CDC paths between every pair. These are safe because:
#   - BUFGMUX handles glitch-free switching
#   - vec_clk_d1 in io_bank is the synchronizer for vec_clk → delay_clk
#   - Config registers (pin_type, pulse_ctrl, freq_sel) are quasi-static
#
# Strategy: false_path all MMCM inter-clock crossings. The only real CDC
# (vec_clk edge detection in io_bank) is a 2-FF synchronizer.

# clk_wiz MMCM outputs — CDC false paths between vec_clk and AXI/delay domains
# Use Vivado's auto-generated clock names (clk_out1_clk_wiz_0, etc.)
# The error BRAMs are dual-port (port A = vec_clk, port B = clk_fpga_0).
# Config registers are quasi-static (written during setup, stable during execution).

# vec_clk (clk_out1) ↔ AXI domain (clk_fpga_0): error BRAM CDC, quasi-static config
set_false_path -from [get_clocks clk_fpga_0] -to [get_clocks clk_out1_clk_wiz_0]
set_false_path -from [get_clocks clk_out1_clk_wiz_0] -to [get_clocks clk_fpga_0]

# vec_clk (clk_out1) ↔ delay domain (clk_fpga_1): io_cell pipeline synchronizer
set_false_path -from [get_clocks clk_fpga_1] -to [get_clocks clk_out1_clk_wiz_0]
set_false_path -from [get_clocks clk_out1_clk_wiz_0] -to [get_clocks clk_fpga_1]

# All other clk_wiz outputs (clk_out2-7) ↔ PS clocks: fixed frequency, no runtime CDC
set_false_path -from [get_clocks clk_fpga_0] -to [get_clocks clk_out2_clk_wiz_0]
set_false_path -from [get_clocks clk_out2_clk_wiz_0] -to [get_clocks clk_fpga_0]
set_false_path -from [get_clocks clk_fpga_1] -to [get_clocks clk_out2_clk_wiz_0]
set_false_path -from [get_clocks clk_out2_clk_wiz_0] -to [get_clocks clk_fpga_1]

# AXI ↔ delay_clk: io_config registers written from AXI, read from delay_clk.
# Written during configuration only, stable during vector execution.
set_false_path -from [get_clocks clk_fpga_0] -to [get_clocks clk_fpga_1]
set_false_path -from [get_clocks clk_fpga_1] -to [get_clocks clk_fpga_0]

# freq_sel and vec_clk_en: quasi-static, BUFGMUX handles switching
set_false_path -from [get_cells u_clk_ctrl/freq_sel_reg[*]]
set_false_path -from [get_cells u_clk_ctrl/vec_clk_en_reg]

# io_config pin type + pulse ctrl registers: written from AXI, read from delay_clk
set_false_path -from [get_cells {u_fbc_top/u_io_config/pin_type_reg_reg[*]}]
set_false_path -from [get_cells {u_fbc_top/u_io_config/pulse_ctrl_reg_reg[*][*]}]
