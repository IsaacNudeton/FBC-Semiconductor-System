set root_dir [file normalize [file dirname [info script]]/..]

puts "============================================"
puts " Bitstream from routed checkpoint"
puts "============================================"

open_checkpoint "$root_dir/build/vivado/fbc_system.runs/impl_1/system_top_routed.dcp"

# Reports
report_utilization -file "$root_dir/build/impl_utilization.rpt"
report_timing_summary -file "$root_dir/build/impl_timing.rpt" -delay_type min_max
report_power -file "$root_dir/build/impl_power.rpt"
report_io -file "$root_dir/build/impl_io.rpt"
report_clock_utilization -file "$root_dir/build/impl_clocks.rpt"
report_drc -file "$root_dir/build/impl_drc.rpt"

# Check timing
set wns [get_property SLACK [get_timing_paths -max_paths 1 -setup]]
puts "  WNS: ${wns} ns"

# Generate bitstream
set_property BITSTREAM.GENERAL.COMPRESS TRUE [current_design]
write_bitstream -force "$root_dir/build/fbc_system.bit"
write_bitstream -force -bin_file "$root_dir/build/fbc_system"

puts ""
puts "============================================"
puts " BUILD SUCCESSFUL"
puts "============================================"
puts " Bitstream: build/fbc_system.bit"
puts " Binary:    build/fbc_system.bin"
puts "============================================"
