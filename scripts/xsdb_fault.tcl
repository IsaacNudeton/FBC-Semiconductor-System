# xsdb_fault.tcl — Full reset + probe fault state
connect -url tcp:localhost:3121

# System reset to recover DAP
targets -set 1
rst -system
after 1000

puts "Targets after reset:"
puts [targets]

# Target ARM core #0
targets -set -filter {name =~ "ARM*#0"}
catch {stop}
after 200

configparams force-mem-accesses 1

# Load firmware again (reset wiped it)
set root_dir [file dirname [file dirname [info script]]]
source "$root_dir/build/ps7_init.tcl"
ps7_init
targets -set -filter {name =~ "xc7z020"}
fpga "$root_dir/build/fbc_system.bit"
targets -set -filter {name =~ "ARM*#0"}
ps7_post_config
dow "$root_dir/firmware/target/armv7a-none-eabi/release/fbc-firmware"

# Run for 3 seconds to let firmware crash
puts "\n>>> Running firmware for 3 seconds..."
con
after 3000

# Stop and inspect
catch {stop}
after 200

puts "\n=== CPU STATE ==="
puts [rrd]

puts "\n=== CP15 ==="
catch {puts [rrd cp15]}

puts "\n=== MEMORY AT PC AREA ==="
set pc_val [lindex [split [rrd pc] :] 1]
puts "PC area:"
mrd 0x00100060 16

puts "\n=== VECTOR TABLE ==="
mrd 0x00100000 8

puts "\n=== OCM TEST ==="
puts "OCM_CFG: [mrd 0xF8000910]"
catch {mrd 0xFFFC0000} r0
puts "0xFFFC0000: $r0"
catch {mrd 0xFFFD0000} r1
puts "0xFFFD0000: $r1"
catch {mrd 0xFFFE0000} r2
puts "0xFFFE0000: $r2"
catch {mrd 0xFFFF0000} r3
puts "0xFFFF0000: $r3"

puts "\n=== KEY FIRMWARE STATE ==="
# Check if firmware wrote anything to UART (UART0 TX FIFO empty?)
puts "UART0 SR: [mrd 0xE0000014]"
# Check GEM0
puts "GEM0 NET_CTRL: [mrd 0xE000B000]"
# Check MIO 52 (did firmware override?)
puts "MIO 52: [mrd 0xF80007D0]"

con
puts "\n=== Done ==="
