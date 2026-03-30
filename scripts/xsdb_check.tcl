connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
catch {stop}
after 200

# Read PC to see where we're stuck
set pc_val [rrd pc]
puts "PC: $pc_val"

# Read CPSR
set cpsr_val [rrd cpsr]
puts "CPSR: $cpsr_val"

# Check XADC CFG register to see if init happened
puts "\nXADC CFG (0xF8007100):"
puts [mrd 0xF8007100]

puts "\nXADC MCTL (0xF8007118):"
puts [mrd 0xF8007118]

# Try reading XADC temp
puts "\nReading XADC temperature..."
mwr 0xF8007110 0x04000000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04000000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set temp_mc [expr {($raw_val * 503975 / 65536) - 273150}]
puts "TEMP: raw=0x[format %04X $raw_val] → [expr {$temp_mc/1000}].[format %03d [expr {abs($temp_mc) % 1000}]]°C"

# Read VCCINT
mwr 0xF8007110 0x04010000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04010000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCINT: raw=0x[format %04X $raw_val] → ${mv}mV"

# Read VCCAUX
mwr 0xF8007110 0x04020000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04020000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCAUX: raw=0x[format %04X $raw_val] → ${mv}mV"

con
