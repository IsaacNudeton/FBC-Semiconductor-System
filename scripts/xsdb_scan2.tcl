# xsdb_scan2.tcl — Detailed JTAG chain scan
# Usage: xsdb scripts/xsdb_scan2.tcl

puts "=== Connecting to hw_server on localhost:3121 ==="
connect -url tcp:localhost:3121

puts "\n=== JTAG targets ==="
puts [targets]

puts "\n=== JTAG sequence scan ==="
catch {jtag targets} result
puts $result

puts "\n=== Done ==="
