# vivado_xadc_read.tcl — Read XADC via Vivado Hardware Manager (JTAG path)
# This bypasses PS-XADC bridge entirely, goes through FPGA JTAG TAP → XADC DRP

open_hw_manager
connect_hw_server -url localhost:3121
open_hw_target

# List all devices
puts "All devices: [get_hw_devices]"

# Get the FPGA device (xc7z020)
set fpga [get_hw_devices xc7z020*]
puts "FPGA device: $fpga"

# Set it as current
current_hw_device $fpga

# Refresh device to read XADC
refresh_hw_device $fpga

# Create system monitor on the FPGA
create_hw_sysmon -hw_device $fpga

# Refresh sysmon to read values
set sysmon [get_hw_sysmons]
puts "Sysmon: $sysmon"

# Read all properties
puts "\n=== XADC via JTAG DRP ==="

catch {
    set temp [get_property TEMPERATURE [get_hw_sysmons]]
    puts "TEMPERATURE: $temp °C"
} err1
if {$err1 ne ""} { puts "TEMP error: $err1" }

catch {
    set vccint [get_property VCCINT [get_hw_sysmons]]
    puts "VCCINT:      $vccint V"
} err2
if {$err2 ne ""} { puts "VCCINT error: $err2" }

catch {
    set vccaux [get_property VCCAUX [get_hw_sysmons]]
    puts "VCCAUX:      $vccaux V"
} err3
if {$err3 ne ""} { puts "VCCAUX error: $err3" }

catch {
    set vccbram [get_property VCCBRAM [get_hw_sysmons]]
    puts "VCCBRAM:     $vccbram V"
} err4
if {$err4 ne ""} { puts "VCCBRAM error: $err4" }

# List all available sysmon properties
catch {
    set all_props [list_property [get_hw_sysmons]]
    puts "\nAll sysmon properties: $all_props"
}

close_hw_target
disconnect_hw_server
close_hw_manager
puts "\nDone."
