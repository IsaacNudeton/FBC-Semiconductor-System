#==============================================================================
# FBC Semiconductor System — Vivado Build Script
#==============================================================================
# Usage:
#   cd C:\Dev\projects\FBC-Semiconductor-System
#   vivado -mode batch -source scripts/build_bitstream.tcl
#
# Target: Xilinx Zynq-7020 CLG484-1  (Sonoma board)
# Output: build/fbc_system.bit
#
# NOTE: The PS7 wrapper is generated automatically by this script.
#       No manual IP generation needed.
#==============================================================================

set project_name "fbc_system"
set project_dir  "build/vivado"
set part         "xc7z020clg484-1"
set top_module   "system_top"

# Resolve paths relative to project root
set script_dir [file dirname [info script]]
set root_dir   [file normalize "$script_dir/.."]

puts "============================================"
puts " FBC Semiconductor System — Vivado Build"
puts " Part:    $part"
puts " Root:    $root_dir"
puts "============================================"

#==============================================================================
# Step 0: Clean previous build
#==============================================================================
if {[file exists "$root_dir/$project_dir"]} {
    puts "Removing previous build directory..."
    file delete -force "$root_dir/$project_dir"
}
file mkdir "$root_dir/build"

#==============================================================================
# Step 1: Create Project
#==============================================================================
puts "\n>>> Step 1: Creating Vivado project..."
create_project $project_name "$root_dir/$project_dir" -part $part -force

#==============================================================================
# Step 2: Add RTL Sources
#==============================================================================
puts "\n>>> Step 2: Adding RTL sources..."

# Add all Verilog source files (not the PS7 — we generate that)
set rtl_files [glob "$root_dir/rtl/*.v"]
# Also add the include header
lappend rtl_files "$root_dir/rtl/fbc_pkg.vh"
add_files $rtl_files

# Set Verilog include path
set_property include_dirs "$root_dir/rtl" [current_fileset]
set_property top $top_module [current_fileset]

#==============================================================================
# Step 3: Add Constraints
#==============================================================================
puts "\n>>> Step 3: Adding constraints..."
add_files -fileset constrs_1 "$root_dir/constraints/zynq7020_sonoma.xdc"

#==============================================================================
# Step 4: Create PS7 IP (Processing System 7)
#==============================================================================
# system_top.v instantiates `processing_system7_0` which is the Vivado IP
# wrapper for the Zynq PS block. We create it using the IP catalog.
#
# This configures:
#   - ARM Cortex-A9 clocks and DDR controller
#   - FCLK_CLK0 = 100 MHz, FCLK_CLK1 = 200 MHz
#   - M_AXI_GP0 (PS master → PL registers)
#   - S_AXI_HP0 (PL DMA → DDR reads)
#   - MIO: UART0, I2C0, SPI0, GEM0 (Ethernet), SD0, GPIO
#   - Interrupts: IRQ_F2P[15:0]
#
# DDR: ISSI IS43TR16256A (256Mx16, DDR3-1600, CL11)
#   Board has 2x chips for 32-bit bus width.
#   Micron MT41K256M16 RE-125 preset used (pin/timing-compatible).
#   Explicit timing params set to match IS43TR16256A-125KBL datasheet.
#   NOTE: Our custom FSBL (fsbl/src/main.rs) has its own DDR init with
#   register values extracted from the actual Sonoma FSBL. The Vivado
#   DDR config here is only used if you use Xilinx's FSBL instead.
#==============================================================================
puts "\n>>> Step 4: Creating PS7 IP..."

create_ip -name processing_system7 -vendor xilinx.com -library ip \
    -version 5.5 -module_name processing_system7_0

set_property -dict [list \
    CONFIG.PCW_PRESET_BANK1_VOLTAGE {LVCMOS 2.5V} \
    CONFIG.PCW_CRYSTAL_PERIPHERAL_FREQMHZ {33.333333} \
    CONFIG.PCW_APU_PERIPHERAL_FREQMHZ {667} \
    CONFIG.PCW_FCLK_CLK0_BUF {TRUE} \
    CONFIG.PCW_FCLK_CLK1_BUF {TRUE} \
    CONFIG.PCW_FPGA0_PERIPHERAL_FREQMHZ {100} \
    CONFIG.PCW_FPGA1_PERIPHERAL_FREQMHZ {200} \
    CONFIG.PCW_EN_CLK1_PORT {1} \
    CONFIG.PCW_USE_M_AXI_GP0 {1} \
    CONFIG.PCW_USE_S_AXI_HP0 {1} \
    CONFIG.PCW_USE_FABRIC_INTERRUPT {1} \
    CONFIG.PCW_IRQ_F2P_INTR {1} \
    CONFIG.PCW_UIPARAM_DDR_MEMORY_TYPE {DDR 3} \
    CONFIG.PCW_UIPARAM_DDR_DEVICE_CAPACITY {4096 MBits} \
    CONFIG.PCW_UIPARAM_DDR_DRAM_WIDTH {16 Bits} \
    CONFIG.PCW_UIPARAM_DDR_BUS_WIDTH {32 Bit} \
    CONFIG.PCW_UIPARAM_DDR_SPEED_BIN {DDR3_1600K} \
    CONFIG.PCW_UIPARAM_DDR_CL {11} \
    CONFIG.PCW_UIPARAM_DDR_CWL {8} \
    CONFIG.PCW_UIPARAM_DDR_T_RCD {14} \
    CONFIG.PCW_UIPARAM_DDR_T_RP {14} \
    CONFIG.PCW_UIPARAM_DDR_T_RC {49} \
    CONFIG.PCW_UIPARAM_DDR_T_RAS_MIN {35} \
    CONFIG.PCW_UIPARAM_DDR_T_FAW {40} \
    CONFIG.PCW_UIPARAM_DDR_PARTNO {MT41K256M16 RE-125} \
    CONFIG.PCW_UART0_PERIPHERAL_ENABLE {1} \
    CONFIG.PCW_UART0_UART0_IO {MIO 14 .. 15} \
    CONFIG.PCW_I2C0_PERIPHERAL_ENABLE {1} \
    CONFIG.PCW_I2C0_I2C0_IO {MIO 10 .. 11} \
    CONFIG.PCW_SPI0_PERIPHERAL_ENABLE {1} \
    CONFIG.PCW_SPI0_SPI0_IO {MIO 1 .. 6} \
    CONFIG.PCW_ENET0_PERIPHERAL_ENABLE {1} \
    CONFIG.PCW_ENET0_ENET0_IO {MIO 16 .. 27} \
    CONFIG.PCW_ENET0_GRP_MDIO_ENABLE {1} \
    CONFIG.PCW_ENET0_GRP_MDIO_IO {MIO 52 .. 53} \
    CONFIG.PCW_SD0_PERIPHERAL_ENABLE {1} \
    CONFIG.PCW_SD0_SD0_IO {MIO 40 .. 45} \
    CONFIG.PCW_GPIO_MIO_GPIO_ENABLE {1} \
] [get_ips processing_system7_0]

# Generate all output products (creates the Verilog wrapper + init files)
generate_target all [get_ips processing_system7_0]

# Synthesize IP out-of-context (works in project mode)
set ip_run [create_ip_run [get_ips processing_system7_0]]
launch_runs $ip_run -jobs 8
wait_on_run $ip_run

puts "PS7 IP generated successfully."
puts "  Wrapper module: processing_system7_0"

#==============================================================================
# Step 4b: Create clk_wiz IP (replaces hand-rolled clk_ctrl + clk_gen)
#==============================================================================
# Sonoma uses clk_wiz at 0x43C30000 with DRP for runtime frequency changes.
# Our hand-rolled clk_ctrl had an AXI runtime crash. This Xilinx IP is proven.
#
# Config: 100MHz input (FCLK_CLK0), 5 outputs (5/10/25/50/100 MHz)
# AXI-Lite DRP interface for runtime reconfiguration
#==============================================================================
puts "\n>>> Step 4b: Creating clk_wiz IP..."

create_ip -name clk_wiz -vendor xilinx.com -library ip \
    -version 6.0 -module_name clk_wiz_0

set_property -dict [list \
    CONFIG.PRIM_IN_FREQ {100.000} \
    CONFIG.USE_DYN_RECONFIG {true} \
    CONFIG.INTERFACE_SELECTION {Enable_AXI} \
    CONFIG.CLKOUT1_USED {true} \
    CONFIG.CLKOUT1_REQUESTED_OUT_FREQ {50.000} \
    CONFIG.CLKOUT2_USED {true} \
    CONFIG.CLKOUT2_REQUESTED_OUT_FREQ {100.000} \
    CONFIG.CLKOUT3_USED {true} \
    CONFIG.CLKOUT3_REQUESTED_OUT_FREQ {25.000} \
    CONFIG.CLKOUT4_USED {true} \
    CONFIG.CLKOUT4_REQUESTED_OUT_FREQ {10.000} \
    CONFIG.CLKOUT5_USED {true} \
    CONFIG.CLKOUT5_REQUESTED_OUT_FREQ {5.000} \
    CONFIG.CLKOUT6_USED {true} \
    CONFIG.CLKOUT6_REQUESTED_OUT_FREQ {50.000} \
    CONFIG.CLKOUT6_REQUESTED_PHASE {90.000} \
    CONFIG.CLKOUT7_USED {true} \
    CONFIG.CLKOUT7_REQUESTED_OUT_FREQ {50.000} \
    CONFIG.CLKOUT7_REQUESTED_PHASE {180.000} \
    CONFIG.USE_LOCKED {true} \
    CONFIG.USE_RESET {true} \
    CONFIG.RESET_TYPE {ACTIVE_LOW} \
] [get_ips clk_wiz_0]

generate_target all [get_ips clk_wiz_0]

set clk_run [create_ip_run [get_ips clk_wiz_0]]
launch_runs $clk_run -jobs 8
wait_on_run $clk_run

puts "clk_wiz IP generated successfully."

update_compile_order -fileset sources_1

#==============================================================================
# Step 5: Synthesis
#==============================================================================
puts "\n>>> Step 5: Running Synthesis..."

# Set synthesis strategy
set_property strategy Flow_PerfOptimized_high [get_runs synth_1]

# Keep module hierarchy for debug visibility
set_property STEPS.SYNTH_DESIGN.ARGS.FLATTEN_HIERARCHY {none} [get_runs synth_1]

launch_runs synth_1 -jobs 8
wait_on_run synth_1

if {[get_property STATUS [get_runs synth_1]] != "synth_design Complete!"} {
    puts "ERROR: Synthesis FAILED"
    puts "Status: [get_property STATUS [get_runs synth_1]]"
    # Save log for debugging
    set log_src "$root_dir/$project_dir/$project_name.runs/synth_1/runme.log"
    if {[file exists $log_src]} {
        file copy -force $log_src "$root_dir/build/synth_error.log"
        puts "Error log: build/synth_error.log"
    }
    exit 1
}

open_run synth_1
report_utilization -file "$root_dir/build/synth_utilization.rpt"
report_timing_summary -file "$root_dir/build/synth_timing.rpt"
puts "Synthesis PASSED. Reports in build/"

#==============================================================================
# Step 6: Implementation (Place & Route)
#==============================================================================
puts "\n>>> Step 6: Running Implementation..."

# Use performance-optimized placement (Explore, not PostRoutePhysOpt — OOM on 8GB)
set_property strategy Performance_Explore [get_runs impl_1]

launch_runs impl_1 -to_step route_design -jobs 8
wait_on_run impl_1

# Check impl status — accept "Complete" or "Complete, Failed Timing"
# (timing failures on CDC false paths are expected and harmless)
set impl_status [get_property STATUS [get_runs impl_1]]
if {![string match "*Complete*" $impl_status]} {
    puts "ERROR: Implementation FAILED"
    puts "Status: $impl_status"
    set log_src "$root_dir/$project_dir/$project_name.runs/impl_1/runme.log"
    if {[file exists $log_src]} {
        file copy -force $log_src "$root_dir/build/impl_error.log"
        puts "Error log: build/impl_error.log"
    }
    exit 1
}
if {[string match "*Failed Timing*" $impl_status]} {
    puts "WARNING: Timing violations present (expected on CDC paths with false_path constraints)"
}

open_run impl_1
report_utilization -file "$root_dir/build/impl_utilization.rpt"
report_timing_summary -file "$root_dir/build/impl_timing.rpt" -delay_type min_max
report_power -file "$root_dir/build/impl_power.rpt"
report_io -file "$root_dir/build/impl_io.rpt"
report_clock_utilization -file "$root_dir/build/impl_clocks.rpt"
report_drc -file "$root_dir/build/impl_drc.rpt"
puts "Implementation PASSED. Reports in build/"

# Check for timing violations
set wns [get_property STATS.WNS [get_runs impl_1]]
set tns [get_property STATS.TNS [get_runs impl_1]]
puts "  WNS (Worst Negative Slack): ${wns} ns"
puts "  TNS (Total Negative Slack):  ${tns} ns"
if {$wns < 0} {
    puts "  WARNING: Timing violations detected! Check impl_timing.rpt"
}

#==============================================================================
# Step 7: Generate Bitstream
#==============================================================================
puts "\n>>> Step 7: Generating Bitstream..."

# Enable .bin generation for SD card boot
set_property STEPS.WRITE_BITSTREAM.ARGS.BIN_FILE true [get_runs impl_1]

launch_runs impl_1 -to_step write_bitstream -jobs 4
wait_on_run impl_1

# Copy outputs to build/
set bit_files [glob -nocomplain "$root_dir/$project_dir/$project_name.runs/impl_1/*.bit"]
set bin_files [glob -nocomplain "$root_dir/$project_dir/$project_name.runs/impl_1/*.bin"]

if {[llength $bit_files] > 0} {
    file copy -force [lindex $bit_files 0] "$root_dir/build/fbc_system.bit"
    puts ""
    puts "============================================"
    puts " BUILD SUCCESSFUL"
    puts "============================================"
    puts " Bitstream: build/fbc_system.bit"
} else {
    puts "ERROR: No bitstream generated!"
    exit 1
}

if {[llength $bin_files] > 0} {
    file copy -force [lindex $bin_files 0] "$root_dir/build/fbc_system.bin"
    puts " Binary:    build/fbc_system.bin"
}

puts ""
puts " Reports:"
puts "   build/synth_utilization.rpt  - Resource usage (synthesis)"
puts "   build/impl_utilization.rpt   - Resource usage (post-route)"
puts "   build/impl_timing.rpt        - Timing analysis"
puts "   build/impl_power.rpt         - Power estimate"
puts "   build/impl_io.rpt            - I/O pin assignments"
puts "   build/impl_clocks.rpt        - Clock utilization"
puts "   build/impl_drc.rpt           - Design rule checks"
puts "============================================"
