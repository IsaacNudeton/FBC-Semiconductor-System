# xsdb_xadc_enable.tcl — THE FIX: set CFG bit 31 (ENABLE)
# XADCPS_CFG_ENABLE_MASK = 0x80000000
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== BEFORE ==="
puts "CFG:  [mrd 0xF8007100]"
puts "MCTL: [mrd 0xF8007118]"

# Step 1: Clear MCTL reset
mwr 0xF8007118 0x00000000
after 10

# Step 2: Set CFG with ENABLE bit (bit 31) + reasonable settings
# 0x80000000 = ENABLE
# 0x00440000 = CFIFOTH=4, DFIFOTH=4
# 0x00001000 = REDGE
# 0x00000200 = TCKRATE=10 (div 8)
# 0x00000014 = IGAP=20
set cfg_val [expr {0x80000000 | 0x00440000 | 0x00001000 | 0x00000200 | 0x00000014}]
puts "\n>>> Setting CFG = 0x[format %08X $cfg_val] (ENABLE=1)..."
mwr 0xF8007100 $cfg_val
after 100

puts "CFG after: [mrd 0xF8007100]"

# Step 3: Drain any stale RDFIFO
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Step 4: Wait for XADC to start converting
puts ">>> Waiting 2s for conversions..."
after 2000

# Step 5: Read with CORRECT format
# Read = 0x04000000 | (addr << 16)
# Need 2 commands: first sends the read, second (NOOP) pushes result to RDFIFO

puts "\n=== XADC READS WITH ENABLE=1 ==="

# TEMPERATURE (reg 0x00)
mwr 0xF8007110 0x04000000
after 10
mwr 0xF8007110 0x00000000
after 10
set dummy [mrd 0xF8007114]
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
if {$raw_val > 0} {
    set temp_mc [expr {($raw_val * 503975 / 65536) - 273150}]
    puts "TEMP:    raw=0x[format %04X $raw_val] → [expr {$temp_mc/1000}].[format %03d [expr {abs($temp_mc) % 1000}]]°C   *** NON-ZERO! ***"
} else {
    puts "TEMP:    raw=0x0000 (still zero)"
}

# VCCINT (reg 0x01)
mwr 0xF8007110 0x04010000
after 10
mwr 0xF8007110 0x00000000
after 10
set dummy [mrd 0xF8007114]
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCINT:  raw=0x[format %04X $raw_val] → ${mv}mV"

# VCCAUX (reg 0x02)
mwr 0xF8007110 0x04020000
after 10
mwr 0xF8007110 0x00000000
after 10
set dummy [mrd 0xF8007114]
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCAUX:  raw=0x[format %04X $raw_val] → ${mv}mV"

# VCCBRAM (reg 0x06)
mwr 0xF8007110 0x04060000
after 10
mwr 0xF8007110 0x00000000
after 10
set dummy [mrd 0xF8007114]
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCBRAM: raw=0x[format %04X $raw_val] → ${mv}mV"

# CONFIG2 (reg 0x42) — should be 0x0400
mwr 0xF8007110 0x04420000
after 10
mwr 0xF8007110 0x00000000
after 10
set dummy [mrd 0xF8007114]
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
puts "CONFIG2: raw=0x[format %04X $raw_val] (expect 0x0400)"

con
puts "\nDone."
