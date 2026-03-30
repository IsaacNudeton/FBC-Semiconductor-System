# xsdb_deploy.tcl — Full deployment: PS init + bitstream + firmware
# Usage: xsdb scripts/xsdb_deploy.tcl
#
# Replaces: fpga_jtag.py + arm_loader.py (2842 lines of Python)

set root_dir [file dirname [file dirname [info script]]]
set bit_file "$root_dir/build/fbc_system.bit"
set elf_file "$root_dir/firmware/target/armv7a-none-eabi/release/fbc-firmware"
set ps7_init_file "$root_dir/build/ps7_init.tcl"

puts "============================================"
puts " FBC Deploy via xsdb"
puts "============================================"

# Connect to hw_server
puts "\n>>> Connecting..."
connect -url tcp:localhost:3121

puts "\nJTAG chain:"
puts [jtag targets]

# Step 1: System reset
puts "\n>>> Step 1: System reset..."
targets -set 1
rst -system
after 1000

# After reset, targets get re-enumerated
puts "Targets after reset:"
puts [targets]

# Step 2: Target ARM core #0 and halt it
puts "\n>>> Step 2: Halt CPU #0..."
targets -set -filter {name =~ "ARM*#0"}
catch {stop} ;# May already be stopped after reset
after 200

# CRITICAL: Enable forced memory access for PS register init.
# Without this, xsdb blocks writes to DDR controller and other "dangerous" addresses.
# ps7_post_config does this too. Must be set BEFORE ps7_init.
set saved_mode [configparams force-mem-accesses]
configparams force-mem-accesses 1

# Step 3: Initialize PS
puts "\n>>> Step 3: PS initialization (ps7_init)..."
source $ps7_init_file
ps7_init
puts "PS initialized (PLLs, DDR, MIO, clocks)."

# Step 4: Program FPGA bitstream
puts "\n>>> Step 4: Programming bitstream..."
targets -set -filter {name =~ "xc7z020"}
fpga $bit_file
puts "Bitstream loaded."

# Step 5: Post-config (PS-PL level shifters)
puts "\n>>> Step 5: PS-PL post config..."
targets -set -filter {name =~ "ARM*#0"}
ps7_post_config
puts "Level shifters enabled."

# Step 6: Load firmware ELF
puts "\n>>> Step 6: Loading firmware..."
dow $elf_file
puts "Firmware loaded."

# Restore memory access mode
configparams force-mem-accesses $saved_mode

# Step 7: Run
puts "\n>>> Step 7: Starting CPU..."
con
after 1000

puts "\n============================================"
puts " DEPLOYED SUCCESSFULLY"
puts "============================================"
puts [targets]
