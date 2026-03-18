#==============================================================================
# Generate bitstream from existing implementation checkpoint
#==============================================================================
set root_dir [file normalize "[file dirname [info script]]/.."]
set dcp "$root_dir/build/vivado/fbc_system.runs/impl_1/system_top_postroute_physopt.dcp"

puts "Opening checkpoint: $dcp"
open_checkpoint $dcp

# Generate reports
report_utilization -file "$root_dir/build/impl_utilization.rpt"
report_timing_summary -file "$root_dir/build/impl_timing.rpt" -delay_type min_max
report_power -file "$root_dir/build/impl_power.rpt"
report_io -file "$root_dir/build/impl_io.rpt"
report_clock_utilization -file "$root_dir/build/impl_clocks.rpt"
report_drc -file "$root_dir/build/impl_drc.rpt"

puts "\n>>> Generating Bitstream..."
write_bitstream -force "$root_dir/build/fbc_system.bit"
write_bitstream -force -bin_file "$root_dir/build/fbc_system"

if {[file exists "$root_dir/build/fbc_system.bit"]} {
    puts ""
    puts "============================================"
    puts " BUILD SUCCESSFUL"
    puts "============================================"
    puts " Bitstream: build/fbc_system.bit"
    if {[file exists "$root_dir/build/fbc_system.bin"]} {
        puts " Binary:    build/fbc_system.bin"
    }
    puts ""
    puts " Reports:"
    puts "   build/impl_utilization.rpt  - Resource usage"
    puts "   build/impl_timing.rpt       - Timing analysis"
    puts "   build/impl_power.rpt        - Power estimate"
    puts "   build/impl_io.rpt           - I/O pin assignments"
    puts "   build/impl_clocks.rpt       - Clock utilization"
    puts "   build/impl_drc.rpt          - Design rule checks"
    puts "============================================"
} else {
    puts "ERROR: No bitstream generated!"
    exit 1
}
