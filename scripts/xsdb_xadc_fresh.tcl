# xsdb_xadc_fresh.tcl — Fresh deploy, halt before XADC init, probe raw state
connect -url tcp:localhost:3121

# Full system reset
targets -set 1
rst -system
after 1000

targets -set -filter {name =~ "ARM*#0"}
catch {stop}
after 200
configparams force-mem-accesses 1

# PS init + bitstream + firmware load (but DON'T start CPU)
set root_dir [file dirname [file dirname [info script]]]
source "$root_dir/build/ps7_init.tcl"
ps7_init
targets -set -filter {name =~ "xc7z020"}
fpga "$root_dir/build/fbc_system.bit"
targets -set -filter {name =~ "ARM*#0"}
ps7_post_config
dow "$root_dir/firmware/target/armv7a-none-eabi/release/fbc-firmware"

# CPU is loaded but NOT running
puts "\n=== XADC STATE BEFORE FIRMWARE RUNS ==="
puts "PCAP_CLK_CTRL: [mrd 0xF8000168]"
puts "MCTL:          [mrd 0xF8007118]"
puts "MSTS:          [mrd 0xF800710C]"
puts "CFG:           [mrd 0xF8007100]"

# Try reading XADC before firmware
# Drain RDFIFO first
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Send read for TEMPERATURE
mwr 0xF8007110 0x00000000
after 50
set raw_line [mrd 0xF8007114]
puts "TEMP before firmware: $raw_line"

# Send read for CONFIG2 (should be 0x0400 default)
mwr 0xF8007110 0x00420000
after 50
puts "CONFIG2 before firmware: [mrd 0xF8007114]"

# Now clear MCTL and try again
puts "\n>>> Clearing MCTL reset..."
mwr 0xF8007118 0x00000000
after 500

# Drain
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Try reading XADC again
mwr 0xF8007110 0x00000000
after 50
set raw_line [mrd 0xF8007114]
puts "TEMP after MCTL clear: $raw_line"

mwr 0xF8007110 0x00420000
after 50
puts "CONFIG2 after MCTL clear: [mrd 0xF8007114]"

# Now try: setup CFG register for the bridge interface first
# CFG: CFIFOTH=4, DFIFOTH=4, WEDGE=0, REDGE=0, TCKRATE=01 (div4), IGAP=5
puts "\n>>> Configuring PS-XADC bridge (TCKRATE=01)..."
mwr 0xF8007100 0x00440105
after 100

# Drain
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

mwr 0xF8007110 0x00000000
after 100
set raw_line [mrd 0xF8007114]
puts "TEMP with TCKRATE=01: $raw_line"

# Try TCKRATE=00 (fastest)
puts "\n>>> Trying TCKRATE=00..."
mwr 0xF8007100 0x00440005
after 100

for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

mwr 0xF8007110 0x00000000
after 100
set raw_line [mrd 0xF8007114]
puts "TEMP with TCKRATE=00: $raw_line"

# Check: is PCAP actually outputting a clock to XADC?
# Check DEVCFG CTRL for any PCAP-related issues
puts "\n=== DEVCFG STATE ==="
puts "CTRL:   [mrd 0xF8007000]"
puts "STATUS: [mrd 0xF8007014]"
puts "MCTRL:  [mrd 0xF8007080]"

# One more thing: try to read XADC via the JTAG DRP path
# Target the FPGA
puts "\n=== XADC via FPGA JTAG ==="
targets -set -filter {name =~ "xc7z020"}
catch {
    # Some xsdb versions support reading sysmon
    puts "Trying sysmon read..."
    readreg xadc
} result
puts "Result: $result"

# Start firmware
targets -set -filter {name =~ "ARM*#0"}
con
puts "\nDone."
