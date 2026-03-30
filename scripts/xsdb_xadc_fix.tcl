# xsdb_xadc_fix.tcl — Fix XADC: deassert reset, init sequencer, verify readings
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== BEFORE FIX ==="
puts "MCTL:    [mrd 0xF8007118]"
puts "MSTS:    [mrd 0xF800710C]"

# Step 1: Deassert XADC reset (clear bit 4 of MCTL)
puts "\n>>> Step 1: Clearing XADC reset (MCTL bit 4)..."
mwr 0xF8007118 0x00000000
after 10
puts "MCTL after clear: [mrd 0xF8007118]"

# Step 2: Wait for XADC to come out of reset
after 100

# Step 3: Configure XADC sequencer via CMDFIFO
# Write CONFIG0 (reg 0x40):
#   Bits [15:12] = SEQ mode: 0001 = single pass, 0010 = continuous
#   Bits [11:8]  = AVG: 00=none, 01=16, 10=64, 11=256
#   Bits [4:0]   = channel for single channel mode
# For continuous sequencer mode scanning temp+supply:
# CONFIG0 = 0x2000 (SEQ=0010=continuous, rest default)
puts "\n>>> Step 2: Configuring XADC sequencer..."

# Write CONFIG0 = 0x2000 (continuous sequence mode)
# Cmd format: bit26=1 (write), [25:16]=reg, [15:0]=data
set cmd_cfg0 [expr {(1 << 26) | (0x40 << 16) | 0x2000}]
puts "Writing CONFIG0=0x2000 (continuous seq): cmd=[format 0x%08X $cmd_cfg0]"
mwr 0xF8007110 $cmd_cfg0
after 10

# Write CONFIG1 = 0x0000 (calibration bits — leave default)
set cmd_cfg1 [expr {(1 << 26) | (0x41 << 16) | 0x0000}]
mwr 0xF8007110 $cmd_cfg1
after 10

# Write CONFIG2 = 0x0400 (ADCCLK divider = 4, typical)
set cmd_cfg2 [expr {(1 << 26) | (0x42 << 16) | 0x0400}]
mwr 0xF8007110 $cmd_cfg2
after 10

# Write SEQ0 (reg 0x48) = enable channels: temp(0), vccint(1), vccaux(2), vccbram(6)
# Bit 0 = TEMP, Bit 1 = VCCINT, Bit 2 = VCCAUX, Bit 6 = VCCBRAM
# Also Bit 8 = calibration
set seq0_val [expr {(1<<0) | (1<<1) | (1<<2) | (1<<6) | (1<<8)}]
set cmd_seq0 [expr {(1 << 26) | (0x48 << 16) | $seq0_val}]
puts "Writing SEQ0=[format 0x%04X $seq0_val] (temp+vccint+vccaux+vccbram+cal)"
mwr 0xF8007110 $cmd_seq0
after 10

# Write SEQ1 (reg 0x49) = 0 (no aux channels for now)
set cmd_seq1 [expr {(1 << 26) | (0x49 << 16) | 0x0000}]
mwr 0xF8007110 $cmd_seq1
after 10

puts "\n>>> Step 3: Waiting for conversions..."
after 500

# Step 4: Read back config to verify writes took effect
puts "\n=== VERIFY CONFIG ==="
puts "MSTS: [mrd 0xF800710C]"

# Read CONFIG0
mwr 0xF8007110 0x00400000
after 10
puts "CONFIG0: [mrd 0xF8007114]"

# Read CONFIG1
mwr 0xF8007110 0x00410000
after 10
puts "CONFIG1: [mrd 0xF8007114]"

# Read CONFIG2
mwr 0xF8007110 0x00420000
after 10
puts "CONFIG2: [mrd 0xF8007114]"

# Read SEQ0
mwr 0xF8007110 0x00480000
after 10
puts "SEQ0:    [mrd 0xF8007114]"

puts "\n=== READ ADC VALUES ==="

# Read TEMPERATURE (reg 0x00)
mwr 0xF8007110 0x00000000
after 10
puts "TEMP raw:    [mrd 0xF8007114]"

# Read VCCINT (reg 0x01)
mwr 0xF8007110 0x00010000
after 10
puts "VCCINT raw:  [mrd 0xF8007114]"

# Read VCCAUX (reg 0x02)
mwr 0xF8007110 0x00020000
after 10
puts "VCCAUX raw:  [mrd 0xF8007114]"

# Read VCCBRAM (reg 0x06)
mwr 0xF8007110 0x00060000
after 10
puts "VCCBRAM raw: [mrd 0xF8007114]"

# Read MAX_TEMP (reg 0x20)
mwr 0xF8007110 0x00200000
after 10
puts "MAX_TEMP:    [mrd 0xF8007114]"

# Read FLAG (reg 0x3F)
mwr 0xF8007110 0x003F0000
after 10
puts "FLAG:        [mrd 0xF8007114]"

# Decode temperature if non-zero
puts "\n=== DECODE ==="
puts "(If TEMP raw > 0: T(C) = raw * 503.975 / 65536 - 273.15)"
puts "(If VCCINT raw > 0: V(mV) = raw * 3000 / 65536)"

con
puts "\nDone."
