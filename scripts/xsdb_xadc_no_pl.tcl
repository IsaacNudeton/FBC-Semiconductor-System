# xsdb_xadc_no_pl.tcl — Test XADC WITHOUT PL bitstream
# Theory: our bitstream might break XADC DRP
connect -url tcp:localhost:3121

# Full system reset
targets -set 1
rst -system
after 1000

targets -set -filter {name =~ "ARM*#0"}
catch {stop}
after 200
configparams force-mem-accesses 1

# PS init ONLY — no bitstream, no firmware
set root_dir [file dirname [file dirname [info script]]]
source "$root_dir/build/ps7_init.tcl"
ps7_init
after 500

puts "=== XADC WITHOUT BITSTREAM ==="
puts "PCAP_CLK_CTRL: [mrd 0xF8000168]"
puts "MCTL:          [mrd 0xF8007118]"
puts "MSTS:          [mrd 0xF800710C]"
puts "CFG:           [mrd 0xF8007100]"

# Clear MCTL reset
mwr 0xF8007118 0x00000000
after 100

# Drain RDFIFO
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Wait for conversions
puts "\n>>> Waiting 2s for XADC conversions (no PL)..."
after 2000

# Read
puts "\n=== READS WITHOUT PL ==="
mwr 0xF8007110 0x00000000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "TEMP: raw=0x[format %04X $raw_val] (decimal=$raw_val)"

mwr 0xF8007110 0x00010000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "VCCINT: raw=0x[format %04X $raw_val]"

mwr 0xF8007110 0x00020000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "VCCAUX: raw=0x[format %04X $raw_val]"

mwr 0xF8007110 0x00420000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "CONFIG2: raw=0x[format %04X $raw_val] (should be 0x0400 default)"

puts "\n>>> Now loading bitstream..."
targets -set -filter {name =~ "xc7z020"}
fpga "$root_dir/build/fbc_system.bit"
targets -set -filter {name =~ "ARM*#0"}
after 500

puts "\n=== XADC AFTER BITSTREAM ==="
puts "MCTL: [mrd 0xF8007118]"

# Clear MCTL again (bitstream programming may re-trigger reset)
mwr 0xF8007118 0x00000000
after 100

# Drain
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

after 2000

mwr 0xF8007110 0x00000000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "TEMP after PL: raw=0x[format %04X $raw_val]"

mwr 0xF8007110 0x00010000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "VCCINT after PL: raw=0x[format %04X $raw_val]"

mwr 0xF8007110 0x00420000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "CONFIG2 after PL: raw=0x[format %04X $raw_val]"

puts "\nDone."
