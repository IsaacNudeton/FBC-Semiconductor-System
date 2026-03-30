# xsdb_scan.tcl — Scan JTAG chain via hw_server
# Usage: xsdb scripts/xsdb_scan.tcl

puts "Connecting to hw_server..."
connect

puts "\nJTAG targets:"
targets

puts "\nDone."
exit
