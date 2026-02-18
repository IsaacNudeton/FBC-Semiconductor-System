#==============================================================================
# FBC Semiconductor System - Constraints for Zynq 7020
#==============================================================================
# Target: xc7z020clg400-1 (Zynq-7000)
#==============================================================================

#==============================================================================
# Clock Definitions
#==============================================================================

# FCLK_CLK0 from PS (100 MHz)
# This is created by the PS block, just define the period for timing
create_clock -period 10.000 -name clk [get_pins design_1_i/processing_system7_0/FCLK_CLK0]

# Vector clock (derived from PLL, variable frequency)
# Assume max 200 MHz for timing analysis
create_clock -period 5.000 -name vec_clk [get_pins u_vec_clk_pll/clk_out1]

#==============================================================================
# Clock Domain Crossings
#==============================================================================

# Async crossing between clk (100MHz) and vec_clk
set_clock_groups -asynchronous \
    -group [get_clocks clk] \
    -group [get_clocks vec_clk]

#==============================================================================
# GPIO Pin Assignments
#==============================================================================
# These depend on your specific board layout
# Example for 128-pin vector I/O spread across banks

# Bank 34 (VCCO = 3.3V) - Vector pins 0-31
# set_property PACKAGE_PIN <pin> [get_ports {pin_dout[0]}]
# set_property IOSTANDARD LVCMOS33 [get_ports {pin_dout[0]}]

# Bank 35 (VCCO = 3.3V) - Vector pins 32-63
# ...

#==============================================================================
# I/O Standards
#==============================================================================

# Default I/O standard for vector pins
set_property IOSTANDARD LVCMOS33 [get_ports pin_dout[*]]
set_property IOSTANDARD LVCMOS33 [get_ports pin_din[*]]

# Slew rate for high-speed switching
set_property SLEW FAST [get_ports pin_dout[*]]

# Drive strength
set_property DRIVE 8 [get_ports pin_dout[*]]

#==============================================================================
# Timing Constraints
#==============================================================================

# Input delay for pin sampling
set_input_delay -clock vec_clk -max 2.0 [get_ports pin_din[*]]
set_input_delay -clock vec_clk -min 0.5 [get_ports pin_din[*]]

# Output delay for pin driving
set_output_delay -clock vec_clk -max 2.0 [get_ports pin_dout[*]]
set_output_delay -clock vec_clk -min 0.5 [get_ports pin_dout[*]]

#==============================================================================
# False Paths
#==============================================================================

# Configuration registers are slow-changing, don't need tight timing
set_false_path -from [get_cells u_axi_fbc_ctrl/fbc_enable_reg]
set_false_path -from [get_cells u_axi_fbc_ctrl/fbc_reset_reg]

# Pin type configuration (only changes between tests)
set_false_path -from [get_ports pin_type[*]]

#==============================================================================
# Physical Constraints
#==============================================================================

# Pblock for FBC decoder (keep it compact for timing)
# create_pblock pblock_fbc
# add_cells_to_pblock [get_pblocks pblock_fbc] [get_cells u_fbc_decoder]
# resize_pblock [get_pblocks pblock_fbc] -add {SLICE_X0Y0:SLICE_X49Y49}

#==============================================================================
# Debug
#==============================================================================

# Mark signals for ILA debugging if needed
# set_property MARK_DEBUG true [get_nets u_fbc_decoder/state[*]]
# set_property MARK_DEBUG true [get_nets u_vector_engine/error_mask[*]]
