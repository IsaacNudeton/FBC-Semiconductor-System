# xsdb_xadc_deep.tcl — Deep XADC/DEVCFG investigation
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== DEVCFG REGISTERS (0xF8007000) ==="
puts "CTRL     (0xF8007000): [mrd 0xF8007000]"
puts "LOCK     (0xF8007004): [mrd 0xF8007004]"
puts "CFG      (0xF8007008): [mrd 0xF8007008]"
puts "INT_STS  (0xF800700C): [mrd 0xF800700C]"
puts "INT_MASK (0xF8007010): [mrd 0xF8007010]"
puts "STATUS   (0xF8007014): [mrd 0xF8007014]"
puts "DMA_SRC  (0xF8007018): [mrd 0xF8007018]"
puts "DMA_DST  (0xF800701C): [mrd 0xF800701C]"
puts "DMA_LEN  (0xF8007020): [mrd 0xF8007020]"
puts "MCTRL    (0xF8007080): [mrd 0xF8007080]"

puts "\n=== PS-XADC BRIDGE STATE ==="
puts "XADC CFG  (0xF8007100): [mrd 0xF8007100]"
puts "XADC MSTS (0xF800710C): [mrd 0xF800710C]"
puts "XADC MCTL (0xF8007118): [mrd 0xF8007118]"

# Drain RDFIFO completely first
puts "\n=== DRAINING RDFIFO ==="
for {set i 0} {$i < 16} {incr i} {
    set msts_val [string trim [lindex [split [mrd 0xF800710C] :] 1]]
    set msts_num [expr 0x$msts_val]
    set dfifoe [expr {($msts_num >> 8) & 1}]
    if {$dfifoe} {
        puts "  RDFIFO empty after draining $i entries"
        break
    }
    set rd_val [mrd 0xF8007114]
    puts "  Drain $i: $rd_val"
}

puts "\n=== FRESH XADC READS (after drain) ==="
# Now MCTL should be 0 (reset cleared from previous script)
puts "MCTL: [mrd 0xF8007118]"
puts "MSTS: [mrd 0xF800710C]"

# Send read command for TEMPERATURE
puts "\nSending read for TEMPERATURE (reg 0x00)..."
mwr 0xF8007110 0x00000000
after 50

# Check MSTS
puts "MSTS after cmd: [mrd 0xF800710C]"

# Try to read
puts "RDFIFO: [mrd 0xF8007114]"

# Send read for VCCINT
puts "\nSending read for VCCINT (reg 0x01)..."
mwr 0xF8007110 0x00010000
after 50
puts "MSTS after cmd: [mrd 0xF800710C]"
puts "RDFIFO: [mrd 0xF8007114]"

# Send read for VCCAUX
puts "\nSending read for VCCAUX (reg 0x02)..."
mwr 0xF8007110 0x00020000
after 50
puts "RDFIFO: [mrd 0xF8007114]"

# Try the "proper" init sequence from Xilinx docs:
# 1. Reset PS-XADC
# 2. Configure bridge
# 3. Setup sequencer
puts "\n=== FULL REINIT ==="

# Assert reset
mwr 0xF8007118 0x00000010
after 10
puts "MCTL after reset assert: [mrd 0xF8007118]"

# Wait
after 100

# Clear reset
mwr 0xF8007118 0x00000000
after 10
puts "MCTL after reset clear: [mrd 0xF8007118]"

# Reconfigure bridge
# CFG: CFIFOTH=4, DFIFOTH=4, TCKRATE=2, IGAP=5
mwr 0xF8007100 0x00440205
after 10

# Drain any data from reset
for {set i 0} {$i < 16} {incr i} {
    set msts_val [string trim [lindex [split [mrd 0xF800710C] :] 1]]
    set msts_num [expr 0x$msts_val]
    set dfifoe [expr {($msts_num >> 8) & 1}]
    if {$dfifoe} {
        break
    }
    mrd 0xF8007114
}
puts "MSTS after drain: [mrd 0xF800710C]"

# Now try a single read
puts "\n=== SINGLE READ TEST ==="
mwr 0xF8007110 0x00000000
after 100
puts "MSTS: [mrd 0xF800710C]"
set msts_val [string trim [lindex [split [mrd 0xF800710C] :] 1]]
set msts_num [expr 0x$msts_val]
set dfifoe [expr {($msts_num >> 8) & 1}]
set dfifo_lvl [expr {($msts_num >> 12) & 0xF}]
puts "DFIFO_LVL=$dfifo_lvl  DFIFOE=$dfifoe"
if {!$dfifoe} {
    puts "RDFIFO: [mrd 0xF8007114]"
} else {
    puts "ERROR: Data FIFO still empty — XADC not responding to commands"
}

con
puts "\nDone."
