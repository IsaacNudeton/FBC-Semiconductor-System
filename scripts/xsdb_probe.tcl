# xsdb_probe.tcl — Read all critical registers to diagnose firmware state
# Usage: xsdb scripts/xsdb_probe.tcl

connect -url tcp:localhost:3121

# Target ARM core #0
targets -set -filter {name =~ "ARM*#0"}

# Force memory access
set saved [configparams force-mem-accesses]
configparams force-mem-accesses 1

puts "============================================"
puts " FBC Hardware Probe"
puts "============================================"

# 1. CPU State
puts "\n=== CPU STATE ==="
catch {stop} ;# Halt to read PC
after 100
set pc_raw [rrd pc]
set cpsr_raw [rrd cpsr]
puts "PC (raw):   $pc_raw"
puts "CPSR (raw): $cpsr_raw"

# Read exception-related registers
puts "\nDFSR (Data Fault Status):  [mrd 0xFFFF0000]"
catch {
    # Try reading DFSR/DFAR via coprocessor - may not work via mrd
    puts "Checking fault registers..."
}

# 2. MIO Pin State (MDIO pins)
puts "\n=== MIO PINS ==="
puts "MIO 11 (PHY_RESET): [mrd 0xF800072C]"
puts "MIO 16 (GEM0 TX0):  [mrd 0xF8000740]"
puts "MIO 52 (MDIO):      [mrd 0xF80007D0]"
puts "MIO 53 (MDC):       [mrd 0xF80007D4]"

# 3. GEM0 (Ethernet Controller)
puts "\n=== GEM0 ETHERNET ==="
puts "NET_CTRL  (0xE000B000): [mrd 0xE000B000]"
puts "NET_CFG   (0xE000B004): [mrd 0xE000B004]"
puts "NET_STATUS(0xE000B008): [mrd 0xE000B008]"
puts "TX_STATUS (0xE000B014): [mrd 0xE000B014]"
puts "RX_STATUS (0xE000B020): [mrd 0xE000B020]"
puts "TX_QBAR   (0xE000B01C): [mrd 0xE000B01C]"
puts "RX_QBAR   (0xE000B018): [mrd 0xE000B018]"
puts "SPEC_ADDR1_BOT (MAC lo): [mrd 0xE000B088]"
puts "SPEC_ADDR1_TOP (MAC hi): [mrd 0xE000B08C]"

# 4. SLCR Clocks
puts "\n=== CLOCKS ==="
puts "ARM_PLL_CTRL  (0xF8000100): [mrd 0xF8000100]"
puts "DDR_PLL_CTRL  (0xF8000104): [mrd 0xF8000104]"
puts "IO_PLL_CTRL   (0xF8000108): [mrd 0xF8000108]"
puts "ARM_CLK_CTRL  (0xF8000120): [mrd 0xF8000120]"
puts "GEM0_CLK_CTRL (0xF8000140): [mrd 0xF8000140]"
puts "GEM0_RCLK_CTRL(0xF8000138): [mrd 0xF8000138]"
puts "APER_CLK_CTRL (0xF800012C): [mrd 0xF800012C]"
puts "PCAP_CLK_CTRL (0xF8000168): [mrd 0xF8000168]"

# 5. OCM Config
puts "\n=== OCM ==="
puts "OCM_CFG (0xF8000910): [mrd 0xF8000910]"

# 6. FBC AXI Registers (our PL)
puts "\n=== FBC PL REGISTERS ==="
puts "FBC_CTRL VERSION (0x4004001C): [mrd 0x4004001C]"
puts "FBC_CTRL STATUS  (0x40040004): [mrd 0x40040004]"
puts "CLK_CTRL STATUS  (0x40080004): [mrd 0x40080004]"

# 7. DDR test — write/read to verify DDR works
puts "\n=== DDR TEST ==="
mwr 0x00100000 0xDEADBEEF
puts "DDR write 0x00100000 = 0xDEADBEEF"
puts "DDR read  0x00100000 = [mrd 0x00100000]"
mwr 0x00100000 0x12345678
puts "DDR write 0x00100000 = 0x12345678"
puts "DDR read  0x00100000 = [mrd 0x00100000]"

# 8. UART0 check
puts "\n=== UART0 ==="
puts "UART0_CR   (0xE0000000): [mrd 0xE0000000]"
puts "UART0_MR   (0xE0000004): [mrd 0xE0000004]"
puts "UART0_SR   (0xE0000014): [mrd 0xE0000014]"

# Resume CPU
con

configparams force-mem-accesses $saved

puts "\n============================================"
puts " Probe complete. CPU resumed."
puts "============================================"
