# xsdb_net_check.tcl — Deep Ethernet/PHY diagnostics
connect -url tcp:localhost:3121
targets -set -filter {name =~ "ARM*#0"}
configparams force-mem-accesses 1
catch {stop}
after 100

puts "=== GEM0 FULL STATE ==="
puts "NET_CTRL   (0xE000B000): [mrd 0xE000B000]"
puts "NET_CFG    (0xE000B004): [mrd 0xE000B004]"
puts "NET_STATUS (0xE000B008): [mrd 0xE000B008]"
puts "DMA_CFG    (0xE000B010): [mrd 0xE000B010]"
puts "TX_STATUS  (0xE000B014): [mrd 0xE000B014]"
puts "RX_QBAR    (0xE000B018): [mrd 0xE000B018]"
puts "TX_QBAR    (0xE000B01C): [mrd 0xE000B01C]"
puts "RX_STATUS  (0xE000B020): [mrd 0xE000B020]"
puts "INTR_STATUS(0xE000B024): [mrd 0xE000B024]"
puts "PHY_MAINT  (0xE000B034): [mrd 0xE000B034]"
puts "MAC_LO     (0xE000B088): [mrd 0xE000B088]"
puts "MAC_HI     (0xE000B08C): [mrd 0xE000B08C]"

# NET_STATUS bit 1 = MDIO idle, bit 2 = PHY link
puts "\n=== NET_STATUS DECODE ==="
set net_status "0x[string range [mrd 0xE000B008] end-8 end]"
puts "Raw: $net_status"
set ns_val [expr {$net_status}]
puts "  Bit 0 (PCS link): [expr {$ns_val & 1}]"
puts "  Bit 1 (MDIO idle): [expr {($ns_val >> 1) & 1}]"
puts "  Bit 2 (PHY mgmt idle): [expr {($ns_val >> 2) & 1}]"

# Read PHY registers via MDIO (manual PHY_MAINT read)
# PHY_MAINT register format:
#   [31:30] = 01 (start), [29:28] = 10 (read), [27:23] = PHY addr, [22:18] = reg, [17:16] = 10
# Read PHY 0 register 0 (Basic Control)
puts "\n=== PHY MDIO READ ==="
# Write PHY_MAINT: start=01, read=10, phy=0, reg=0, must10=10
# = 0x60000000 | (0 << 23) | (0 << 18) | (2 << 16)
set phy_read_cmd [expr {0x60020000}]
puts "Sending MDIO read cmd: [format 0x%08X $phy_read_cmd]"
mwr 0xE000B034 $phy_read_cmd
after 50
puts "PHY_MAINT after read: [mrd 0xE000B034]"
puts "NET_STATUS after read: [mrd 0xE000B008]"

# Read PHY 0 register 1 (Basic Status - has link bit)
set phy_status_cmd [expr {0x60020000 | (1 << 18)}]
mwr 0xE000B034 $phy_status_cmd
after 50
puts "PHY reg 1 (Status): [mrd 0xE000B034]"

# Read PHY 0 register 2 (PHY ID1)
set phy_id1_cmd [expr {0x60020000 | (2 << 18)}]
mwr 0xE000B034 $phy_id1_cmd
after 50
puts "PHY reg 2 (ID1): [mrd 0xE000B034]"

# Read PHY 0 register 3 (PHY ID2)
set phy_id2_cmd [expr {0x60020000 | (3 << 18)}]
mwr 0xE000B034 $phy_id2_cmd
after 50
puts "PHY reg 3 (ID2): [mrd 0xE000B034]"

# Try PHY address 1 (some PHYs use addr 1)
set phy1_status [expr {0x60020000 | (1 << 23) | (1 << 18)}]
mwr 0xE000B034 $phy1_status
after 50
puts "PHY@1 reg 1 (Status): [mrd 0xE000B034]"

# Check TX descriptor ring
puts "\n=== TX DESCRIPTORS ==="
puts "TX_QBAR (descriptor base): [mrd 0xE000B01C]"
set tx_base "0x[string range [mrd 0xE000B01C] end-8 end]"
set tx_addr [expr {$tx_base}]
if {$tx_addr != 0} {
    puts "TX Desc 0 (addr):  [mrd $tx_addr]"
    puts "TX Desc 0 (ctrl):  [mrd [expr {$tx_addr + 4}]]"
    puts "TX Desc 1 (addr):  [mrd [expr {$tx_addr + 8}]]"
    puts "TX Desc 1 (ctrl):  [mrd [expr {$tx_addr + 12}]]"
}

puts "\n=== RX DESCRIPTORS ==="
puts "RX_QBAR (descriptor base): [mrd 0xE000B018]"

con
puts "\nDone."
