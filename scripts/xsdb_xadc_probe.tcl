# xsdb_xadc_probe.tcl — Deep XADC register probe on live hardware
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== PCAP CLOCK ==="
puts "PCAP_CLK_CTRL (0xF8000168): [mrd 0xF8000168]"
puts "APER_CLK_CTRL (0xF800012C): [mrd 0xF800012C]"

puts "\n=== PS-XADC BRIDGE REGISTERS (0xF8007100) ==="
puts "CFG     (0xF8007100): [mrd 0xF8007100]"
puts "INT_STS (0xF8007104): [mrd 0xF8007104]"
puts "INT_MASK(0xF8007108): [mrd 0xF8007108]"
puts "MSTS    (0xF800710C): [mrd 0xF800710C]"
puts "MCTL    (0xF8007118): [mrd 0xF8007118]"

puts "\n=== XADC INTERNAL REGISTERS (via CMDFIFO/RDFIFO) ==="

# Helper: write read command, wait, read result
# Command format: [25:16] = register address, bit26=0 for read

# Read TEMPERATURE (reg 0x00)
mwr 0xF8007110 0x00000000
after 10
puts "TEMP raw:   [mrd 0xF8007114]"

# Read VCCINT (reg 0x01)
mwr 0xF8007110 0x00010000
after 10
puts "VCCINT raw: [mrd 0xF8007114]"

# Read VCCAUX (reg 0x02)
mwr 0xF8007110 0x00020000
after 10
puts "VCCAUX raw: [mrd 0xF8007114]"

# Read VP/VN (reg 0x03)
mwr 0xF8007110 0x00030000
after 10
puts "VP/VN raw:  [mrd 0xF8007114]"

# Read VREFP (reg 0x04)
mwr 0xF8007110 0x00040000
after 10
puts "VREFP raw:  [mrd 0xF8007114]"

# Read VREFN (reg 0x05)
mwr 0xF8007110 0x00050000
after 10
puts "VREFN raw:  [mrd 0xF8007114]"

# Read VCCBRAM (reg 0x06)
mwr 0xF8007110 0x00060000
after 10
puts "VCCBRAM raw:[mrd 0xF8007114]"

puts "\n=== XADC CONFIG REGISTERS ==="

# CONFIG0 (reg 0x40) - averaging, channel, sequencer mode
mwr 0xF8007110 0x00400000
after 10
puts "CONFIG0 (0x40): [mrd 0xF8007114]"

# CONFIG1 (reg 0x41) - sequencer selection, calibration
mwr 0xF8007110 0x00410000
after 10
puts "CONFIG1 (0x41): [mrd 0xF8007114]"

# CONFIG2 (reg 0x42) - clock division, power down
mwr 0xF8007110 0x00420000
after 10
puts "CONFIG2 (0x42): [mrd 0xF8007114]"

puts "\n=== SEQUENCER REGISTERS ==="

# SEQ0 (reg 0x48) - channel enable for averaging
mwr 0xF8007110 0x00480000
after 10
puts "SEQ0 (0x48): [mrd 0xF8007114]"

# SEQ1 (reg 0x49)
mwr 0xF8007110 0x00490000
after 10
puts "SEQ1 (0x49): [mrd 0xF8007114]"

# SEQ2 (reg 0x4A) - input mode select
mwr 0xF8007110 0x004A0000
after 10
puts "SEQ2 (0x4A): [mrd 0xF8007114]"

# SEQ3 (reg 0x4B) - input mode
mwr 0xF8007110 0x004B0000
after 10
puts "SEQ3 (0x4B): [mrd 0xF8007114]"

puts "\n=== FLAG REGISTER ==="
# FLAG (reg 0x3F) - JTAG locked, ref, over-temp, alarm
mwr 0xF8007110 0x003F0000
after 10
puts "FLAG (0x3F): [mrd 0xF8007114]"

puts "\n=== MSTS after all reads ==="
puts "MSTS: [mrd 0xF800710C]"

con
puts "\nDone."
