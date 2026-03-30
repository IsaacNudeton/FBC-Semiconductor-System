# xsdb_xadc_test.tcl — Test XADC after proper init
# Theory: MCTL resets to 0x10 (bit 4 = XADC reset asserted by default)
# Our firmware never clears it → XADC stuck in reset forever
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== THEORY: MCTL POR default = 0x10 (reset asserted) ==="
puts "Current MCTL: [mrd 0xF8007118]"

# Step 1: Clear MCTL (deassert XADC reset)
puts "\n>>> Clearing XADC reset..."
mwr 0xF8007118 0x00000000
after 10

# Step 2: Drain any stale RDFIFO data
for {set i 0} {$i < 16} {incr i} {
    set raw [mrd 0xF800710C]
    set val [expr 0x[string trim [lindex [split $raw :] 1]]]
    if {($val >> 8) & 1} break
    mrd 0xF8007114
}

# Step 3: Wait a FULL SECOND for XADC default mode conversions
puts ">>> Waiting 2 seconds for XADC default mode conversions..."
after 2000

# Step 4: Read — in DEFAULT mode (CONFIG0=0), XADC auto-scans temp/supply
puts "\n=== XADC READS (default mode, no sequencer config) ==="

# TEMPERATURE (reg 0x00)
mwr 0xF8007110 0x00000000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set temp_mc [expr {($raw_val * 503975 / 65536) - 273150}]
puts "TEMP:    raw=0x[format %04X $raw_val]  ($raw_val) → [expr {$temp_mc/1000}].[format %03d [expr {abs($temp_mc) % 1000}]]°C"

# VCCINT (reg 0x01)
mwr 0xF8007110 0x00010000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCINT:  raw=0x[format %04X $raw_val]  → ${mv}mV"

# VCCAUX (reg 0x02)
mwr 0xF8007110 0x00020000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCAUX:  raw=0x[format %04X $raw_val]  → ${mv}mV"

# VCCBRAM (reg 0x06)
mwr 0xF8007110 0x00060000
after 50
set raw_line [mrd 0xF8007114]
set raw_hex [string trim [lindex [split $raw_line :] 1]]
set raw_val [expr 0x$raw_hex]
set mv [expr {$raw_val * 3000 / 65536}]
puts "VCCBRAM: raw=0x[format %04X $raw_val]  → ${mv}mV"

# VP/VN (reg 0x03)
mwr 0xF8007110 0x00030000
after 50
set raw_line [mrd 0xF8007114]
puts "VP/VN:   $raw_line"

# MAX_TEMP (reg 0x20) — should be non-zero if ANY conversion happened
mwr 0xF8007110 0x00200000
after 50
set raw_line [mrd 0xF8007114]
puts "MAX_TEMP: $raw_line"

# CONFIG0 (reg 0x40)
mwr 0xF8007110 0x00400000
after 50
puts "CONFIG0: [mrd 0xF8007114]"

# CONFIG1 (reg 0x41)
mwr 0xF8007110 0x00410000
after 50
puts "CONFIG1: [mrd 0xF8007114]"

# CONFIG2 (reg 0x42) — default should be 0x0400
mwr 0xF8007110 0x00420000
after 50
puts "CONFIG2: [mrd 0xF8007114]"

# FLAG (reg 0x3F)
mwr 0xF8007110 0x003F0000
after 50
puts "FLAG:    [mrd 0xF8007114]"

puts "\nMSTS: [mrd 0xF800710C]"

con
puts "\nDone."
