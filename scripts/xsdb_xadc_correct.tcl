# xsdb_xadc_correct.tcl — XADC with CORRECT command format
# Read = 0x04000000 | (addr << 16)
# Write = 0x08000000 | (addr << 16) | data
# Blog ref: http://henryomd.blogspot.com/2015/06/bare-metal-code-to-read-adc-on-zynq.html

connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== USING CORRECT CMDFIFO FORMAT ==="

# Clear MCTL (deassert XADC reset)
mwr 0xF8007118 0x00000000
after 100

# Drain RDFIFO
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Wait for XADC to start converting in default mode
after 1000

puts "\n=== CORRECT READ COMMANDS (0x04NN0000) ==="

# Read TEMPERATURE (reg 0x00): 0x04000000
mwr 0xF8007110 0x04000000
after 50
# Blog says: read twice, discard first, keep second
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04000000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set temp_mc [expr {($raw_val * 503975 / 65536) - 273150}]
puts "TEMP:    raw=0x[format %04X $raw_val] → [expr {$temp_mc/1000}].[format %03d [expr {abs($temp_mc) % 1000}]]°C"

# Read VCCINT (reg 0x01): 0x04010000
mwr 0xF8007110 0x04010000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04010000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCINT:  raw=0x[format %04X $raw_val] → ${mv}mV"

# Read VCCAUX (reg 0x02): 0x04020000
mwr 0xF8007110 0x04020000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04020000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCAUX:  raw=0x[format %04X $raw_val] → ${mv}mV"

# Read VCCBRAM (reg 0x06): 0x04060000
mwr 0xF8007110 0x04060000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04060000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCBRAM: raw=0x[format %04X $raw_val] → ${mv}mV"

# Read CONFIG2 (reg 0x42): 0x04420000 — should be 0x0400 default
mwr 0xF8007110 0x04420000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04420000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "CONFIG2: raw=0x[format %04X $raw_val] (should be 0x0400)"

# Read CONFIG0 (reg 0x40): 0x04400000
mwr 0xF8007110 0x04400000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x04400000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "CONFIG0: raw=0x[format %04X $raw_val]"

# Read FLAG (reg 0x3F): 0x043F0000
mwr 0xF8007110 0x043F0000
after 50
set dummy [mrd 0xF8007114]
mwr 0xF8007110 0x043F0000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "FLAG:    raw=0x[format %04X $raw_val]"

con
puts "\nDone."
