//! Network Interface
//!
//! Zynq GEM Ethernet driver for FBC burn-in system.
//! Raw Ethernet + UDP only - no TCP overhead.
//!
//! # Design (ONETWO)
//! - Invariant: Switch knows MAC→port mapping
//! - Varies: Which controllers are online
//! - Pattern: Query switch (SNMP), not controllers (DHCP)
//!
//! Speed: 100ms boot, <1ms broadcast to 500 boards

use core::ptr::{read_volatile, write_volatile};
use crate::hal::{mac_from_dna, ip_from_dna};

// =============================================================================
// Zynq GEM Registers (Xilinx ug585)
// =============================================================================

pub const GEM0_BASE: usize = 0xE000_B000;

// Control registers
const NET_CTRL: usize = 0x00;
const NET_CFG: usize = 0x04;
const NET_STATUS: usize = 0x08;
const DMA_CFG: usize = 0x10;
const TX_STATUS: usize = 0x14;
const RX_QBAR: usize = 0x18;
const TX_QBAR: usize = 0x1C;
const RX_STATUS: usize = 0x20;
const INTR_STATUS: usize = 0x24;
const INTR_EN: usize = 0x28;
const INTR_DIS: usize = 0x2C;
const PHY_MAINT: usize = 0x34;
const HASH_BOT: usize = 0x80;
const HASH_TOP: usize = 0x84;
const SPEC_ADDR1_BOT: usize = 0x88;  // MAC address low
const SPEC_ADDR1_TOP: usize = 0x8C;  // MAC address high

// Control bits
const CTRL_TXEN: u32 = 1 << 3;
const CTRL_RXEN: u32 = 1 << 2;
const CTRL_MGMT_PORT_EN: u32 = 1 << 4;
const CTRL_CLR_STAT: u32 = 1 << 5;

// Config bits
const CFG_SPEED: u32 = 1 << 0;       // 1 = 100Mbps
const CFG_FD: u32 = 1 << 1;          // Full duplex
const CFG_COPY_ALL: u32 = 1 << 4;    // Copy all frames
const CFG_NO_BCAST: u32 = 1 << 5;    // Don't copy broadcast
const CFG_MULTI_HASH: u32 = 1 << 6;  // Multicast hash enable
const CFG_UNI_HASH: u32 = 1 << 7;    // Unicast hash enable
const CFG_RX_1536: u32 = 1 << 8;     // Receive 1536 byte frames
const CFG_GIG_EN: u32 = 1 << 10;     // Gigabit mode
const CFG_PCS_SEL: u32 = 1 << 11;    // PCS select
const CFG_MDC_CLK_DIV: u32 = 0x7 << 18; // MDC clock divisor

// Status bits
const STATUS_MGMT_IDLE: u32 = 1 << 2;

// MDIO/PHY constants
const MDIO_OP_READ: u32 = 0b10 << 28;
const MDIO_OP_WRITE: u32 = 0b01 << 28;
const MDIO_CLAUSE22: u32 = 0b01 << 30;
const MDIO_MUST_10: u32 = 0b10 << 16;

// PHY registers (IEEE 802.3)
const PHY_BMCR: u8 = 0;      // Basic Mode Control
const PHY_BMSR: u8 = 1;      // Basic Mode Status
const PHY_ID1: u8 = 2;       // PHY ID 1
const PHY_ID2: u8 = 3;       // PHY ID 2
const PHY_ANAR: u8 = 4;      // Auto-Neg Advertisement
const PHY_ANLPAR: u8 = 5;    // Auto-Neg Link Partner Ability
const PHY_ANER: u8 = 6;      // Auto-Neg Expansion
const PHY_GBCR: u8 = 9;      // 1000BASE-T Control
const PHY_GBSR: u8 = 10;     // 1000BASE-T Status

// BMCR bits
const BMCR_RESET: u16 = 1 << 15;
const BMCR_LOOPBACK: u16 = 1 << 14;
const BMCR_SPEED_100: u16 = 1 << 13;
const BMCR_AN_ENABLE: u16 = 1 << 12;
const BMCR_POWER_DOWN: u16 = 1 << 11;
const BMCR_ISOLATE: u16 = 1 << 10;
const BMCR_RESTART_AN: u16 = 1 << 9;
const BMCR_FULL_DUPLEX: u16 = 1 << 8;
const BMCR_SPEED_1000: u16 = 1 << 6;

// BMSR bits
const BMSR_LINK_STATUS: u16 = 1 << 2;
const BMSR_AN_COMPLETE: u16 = 1 << 5;

// =============================================================================
// Buffer Descriptors
// =============================================================================

/// RX buffer descriptor (8 bytes)
#[repr(C, packed)]
pub struct RxBd {
    pub addr: u32,
    pub status: u32,
}

/// TX buffer descriptor (8 bytes)
#[repr(C, packed)]
pub struct TxBd {
    pub addr: u32,
    pub status: u32,
}

// RX BD status bits
const RX_BD_USED: u32 = 1 << 0;
const RX_BD_WRAP: u32 = 1 << 1;
const RX_BD_SOF: u32 = 1 << 14;
const RX_BD_EOF: u32 = 1 << 15;
const RX_BD_LEN_MASK: u32 = 0x1FFF;

// TX BD status bits
const TX_BD_USED: u32 = 1 << 31;
const TX_BD_WRAP: u32 = 1 << 30;
const TX_BD_LAST: u32 = 1 << 15;
const TX_BD_LEN_MASK: u32 = 0x3FFF;

// =============================================================================
// Network Configuration
// =============================================================================

/// Network configuration
#[derive(Clone)]
pub struct NetConfig {
    pub mac: [u8; 6],
    pub ip: [u8; 4],
    pub gateway: [u8; 4],
    pub netmask: [u8; 4],
    pub port: u16,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self::from_dna()
    }
}

impl NetConfig {
    /// Generate config from device DNA (unique per board)
    pub fn from_dna() -> Self {
        Self {
            mac: mac_from_dna(),        // Unique MAC from DNA
            ip: ip_from_dna(),          // Static IP from DNA (172.16.x.x)
            gateway: [172, 16, 0, 1],
            netmask: [255, 255, 0, 0],  // /16 subnet (65k boards)
            port: 3000,                  // FBC protocol port
        }
    }
}

// =============================================================================
// Ethernet Driver
// =============================================================================

// Buffer ring sizes (must be power of 2)
const RX_BD_COUNT: usize = 32;
const TX_BD_COUNT: usize = 32;
const RX_BUF_SIZE: usize = 1536;
const TX_BUF_SIZE: usize = 1536;

/// Zynq GEM Ethernet driver
pub struct GemEth {
    base: usize,
    rx_bd_ring: *mut RxBd,
    tx_bd_ring: *mut TxBd,
    rx_buffers: *mut u8,
    tx_buffers: *mut u8,
    rx_index: usize,
    tx_index: usize,
    mac: [u8; 6],  // Store our MAC address for FBC protocol
}

// Buffer memory in OCM (after DMA buffers)
const RX_BD_BASE: usize = 0xFFFD_0000;
const TX_BD_BASE: usize = RX_BD_BASE + RX_BD_COUNT * 8;
const RX_BUF_BASE: usize = TX_BD_BASE + TX_BD_COUNT * 8;
const TX_BUF_BASE: usize = RX_BUF_BASE + RX_BD_COUNT * RX_BUF_SIZE;

impl GemEth {
    pub const fn new() -> Self {
        Self {
            base: GEM0_BASE,
            rx_bd_ring: RX_BD_BASE as *mut RxBd,
            tx_bd_ring: TX_BD_BASE as *mut TxBd,
            rx_buffers: RX_BUF_BASE as *mut u8,
            tx_buffers: TX_BUF_BASE as *mut u8,
            rx_index: 0,
            tx_index: 0,
            mac: [0; 6],
        }
    }

    /// Initialize the GEM controller
    pub fn init(&mut self, config: &NetConfig) {
        // Store MAC address
        self.mac = config.mac;

        // Disable RX/TX
        self.write_reg(NET_CTRL, 0);

        // Clear statistics
        self.write_reg(NET_CTRL, CTRL_CLR_STAT);

        // Configure for 100Mbps full duplex (adjust for your PHY)
        let cfg = CFG_SPEED | CFG_FD | CFG_RX_1536 | (0x4 << 18);
        self.write_reg(NET_CFG, cfg);

        // DMA configuration
        self.write_reg(DMA_CFG, 0x00180704);  // Default for Zynq

        // Set MAC address
        self.set_mac_address(&config.mac);

        // Initialize buffer descriptors
        self.init_rx_bd();
        self.init_tx_bd();

        // Set BD ring addresses
        self.write_reg(RX_QBAR, RX_BD_BASE as u32);
        self.write_reg(TX_QBAR, TX_BD_BASE as u32);

        // Enable RX/TX and MDIO
        self.write_reg(NET_CTRL, CTRL_TXEN | CTRL_RXEN | CTRL_MGMT_PORT_EN);

        // Initialize PHY (KSZ9021 or compatible)
        // This does auto-negotiation and waits for link
        self.phy_init();
    }

    /// Set MAC address
    fn set_mac_address(&self, mac: &[u8; 6]) {
        let bot = (mac[0] as u32)
            | ((mac[1] as u32) << 8)
            | ((mac[2] as u32) << 16)
            | ((mac[3] as u32) << 24);
        let top = (mac[4] as u32) | ((mac[5] as u32) << 8);

        self.write_reg(SPEC_ADDR1_BOT, bot);
        self.write_reg(SPEC_ADDR1_TOP, top);
    }

    /// Initialize RX buffer descriptors
    fn init_rx_bd(&mut self) {
        for i in 0..RX_BD_COUNT {
            let bd = unsafe { &mut *self.rx_bd_ring.add(i) };
            let buf_addr = RX_BUF_BASE + i * RX_BUF_SIZE;

            bd.addr = buf_addr as u32;
            bd.status = 0;

            // Mark last BD with wrap bit
            if i == RX_BD_COUNT - 1 {
                bd.addr |= RX_BD_WRAP;
            }
        }
        self.rx_index = 0;
        
        // Memory barrier to ensure DMA sees descriptor updates
        // Cortex-A9 L1 cache is coherent with DMA for OCM region (0xFFFDxxxx)
        // but we need to ensure writes complete before enabling RX
        unsafe {
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        }
    }

    /// Initialize TX buffer descriptors
    fn init_tx_bd(&mut self) {
        for i in 0..TX_BD_COUNT {
            let bd = unsafe { &mut *self.tx_bd_ring.add(i) };
            let buf_addr = TX_BUF_BASE + i * TX_BUF_SIZE;

            bd.addr = buf_addr as u32;
            bd.status = TX_BD_USED;  // Mark as used (empty)

            // Mark last BD with wrap bit
            if i == TX_BD_COUNT - 1 {
                bd.status |= TX_BD_WRAP;
            }
        }
        self.tx_index = 0;
        
        // Memory barrier to ensure DMA sees descriptor updates
        unsafe {
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        }
    }

    /// Receive a packet
    /// Returns the number of bytes received, or 0 if no packet available
    pub fn recv(&mut self, buffer: &mut [u8]) -> usize {
        let bd = unsafe { &mut *self.rx_bd_ring.add(self.rx_index) };

        // Check if BD has been filled
        if bd.addr & RX_BD_USED == 0 {
            return 0;  // No packet yet
        }

        let len = (bd.status & RX_BD_LEN_MASK) as usize;
        if len > buffer.len() {
            // Packet too large - drop it
            self.release_rx_bd();
            return 0;
        }

        // Copy data from buffer
        let src = unsafe { self.rx_buffers.add(self.rx_index * RX_BUF_SIZE) };
        unsafe {
            core::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), len);
        }

        self.release_rx_bd();
        len
    }

    /// Release current RX BD back to hardware
    fn release_rx_bd(&mut self) {
        let bd = unsafe { &mut *self.rx_bd_ring.add(self.rx_index) };
        let wrap = bd.addr & RX_BD_WRAP;
        let buf_addr = RX_BUF_BASE + self.rx_index * RX_BUF_SIZE;
        bd.addr = (buf_addr as u32) | wrap;
        bd.status = 0;

        self.rx_index = (self.rx_index + 1) % RX_BD_COUNT;
    }

    /// Transmit a packet
    /// Returns true if packet was queued, false if no TX buffers available
    pub fn send(&mut self, data: &[u8]) -> bool {
        if data.len() > TX_BUF_SIZE {
            return false;
        }

        let bd = unsafe { &mut *self.tx_bd_ring.add(self.tx_index) };

        // Check if BD is available
        if bd.status & TX_BD_USED == 0 {
            return false;  // Still pending
        }

        // Copy data to buffer
        let dst = unsafe { self.tx_buffers.add(self.tx_index * TX_BUF_SIZE) };
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }

        // Set up BD
        let wrap = bd.status & TX_BD_WRAP;
        bd.status = (data.len() as u32) | TX_BD_LAST | wrap;

        // Advance index
        self.tx_index = (self.tx_index + 1) % TX_BD_COUNT;

        // Start transmission
        let ctrl = self.read_reg(NET_CTRL);
        self.write_reg(NET_CTRL, ctrl | (1 << 9));  // Start TX

        true
    }

    /// Check if TX is idle
    pub fn tx_idle(&self) -> bool {
        self.read_reg(TX_STATUS) & 0x08 != 0
    }

    // Register access
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }

    // =========================================================================
    // MDIO / PHY Management
    // =========================================================================

    /// Wait for MDIO operation to complete
    fn mdio_wait(&self) {
        for _ in 0..10000 {
            if self.read_reg(NET_STATUS) & STATUS_MGMT_IDLE != 0 {
                return;
            }
        }
    }

    /// Read PHY register via MDIO
    pub fn phy_read(&self, phy_addr: u8, reg: u8) -> u16 {
        self.mdio_wait();

        let cmd = MDIO_CLAUSE22
            | MDIO_OP_READ
            | ((phy_addr as u32 & 0x1F) << 23)
            | ((reg as u32 & 0x1F) << 18)
            | MDIO_MUST_10;

        self.write_reg(PHY_MAINT, cmd);
        self.mdio_wait();

        (self.read_reg(PHY_MAINT) & 0xFFFF) as u16
    }

    /// Write PHY register via MDIO
    pub fn phy_write(&self, phy_addr: u8, reg: u8, val: u16) {
        self.mdio_wait();

        let cmd = MDIO_CLAUSE22
            | MDIO_OP_WRITE
            | ((phy_addr as u32 & 0x1F) << 23)
            | ((reg as u32 & 0x1F) << 18)
            | MDIO_MUST_10
            | (val as u32);

        self.write_reg(PHY_MAINT, cmd);
        self.mdio_wait();
    }

    /// Detect PHY address by scanning MDIO bus
    pub fn phy_detect(&self) -> Option<u8> {
        for addr in 0..32u8 {
            let id1 = self.phy_read(addr, PHY_ID1);
            let id2 = self.phy_read(addr, PHY_ID2);

            // Valid PHY IDs are non-zero and not 0xFFFF
            if id1 != 0 && id1 != 0xFFFF && id2 != 0xFFFF {
                return Some(addr);
            }
        }
        None
    }

    /// Initialize PHY (KSZ9021 or compatible)
    /// Returns true if link is up
    pub fn phy_init(&self) -> bool {
        // Detect PHY address
        let phy_addr = match self.phy_detect() {
            Some(addr) => addr,
            None => return false, // No PHY found
        };

        // Soft reset PHY
        self.phy_write(phy_addr, PHY_BMCR, BMCR_RESET);

        // Wait for reset to complete (bit clears)
        for _ in 0..1000 {
            if self.phy_read(phy_addr, PHY_BMCR) & BMCR_RESET == 0 {
                break;
            }
            crate::hal::delay_us(100);
        }

        // Configure RGMII delays for KSZ9021/KSZ9031
        // This is CRITICAL for proper timing on RGMII interface
        self.configure_phy_rgmii_delays(phy_addr);

        // Enable auto-negotiation for 10/100 only
        // DO NOT advertise 1000M — MAC is configured for 100M and SLCR clocks
        // aren't set for 125MHz RGMII. Speed mismatch = no communication.
        self.phy_write(phy_addr, PHY_ANAR, 0x01E1); // 10/100 FD + HD + 802.3
        self.phy_write(phy_addr, PHY_GBCR, 0x0000); // Disable 1000M advertisement

        // Start auto-negotiation
        self.phy_write(phy_addr, PHY_BMCR, BMCR_AN_ENABLE | BMCR_RESTART_AN);

        // Wait for link (up to 5 seconds)
        for _ in 0..50 {
            let status = self.phy_read(phy_addr, PHY_BMSR);
            if status & BMSR_LINK_STATUS != 0 {
                return true;
            }
            crate::hal::delay_ms(100);
        }

        false
    }

    /// Configure RGMII TX/RX clock delays in PHY
    ///
    /// For KSZ9021: Uses extended registers 0x104 (RX) and 0x105 (TX)
    /// For KSZ9031: Uses MMD registers
    ///
    /// Without these delays, RGMII timing is wrong and packets are corrupted.
    fn configure_phy_rgmii_delays(&self, phy_addr: u8) {
        // Read PHY ID to determine type
        let id1 = self.phy_read(phy_addr, PHY_ID1);
        let id2 = self.phy_read(phy_addr, PHY_ID2);

        // Micrel/Microchip PHY detection
        if id1 == 0x0022 {
            // KSZ9021 (ID2 = 0x1611 or 0x1612)
            if (id2 & 0xFFF0) == 0x1610 {
                // KSZ9021 RGMII delays via extended registers
                // Write to extended register 0x104 (RX clock pad skew)
                self.phy_write(phy_addr, 0x0B, 0x8104); // Select ext reg 0x104
                self.phy_write(phy_addr, 0x0C, 0xF0F0); // RX_CLK pad skew = max

                // Write to extended register 0x105 (TX clock pad skew)
                self.phy_write(phy_addr, 0x0B, 0x8105); // Select ext reg 0x105
                self.phy_write(phy_addr, 0x0C, 0x0000); // TX_CLK pad skew = 0 (no delay)
            }
            // KSZ9031 (ID2 = 0x1620-0x162F)
            else if (id2 & 0xFFF0) == 0x1620 {
                // KSZ9031 uses MMD (MDIO Manageable Device) registers
                // MMD 2, register 8: RX_CLK pad skew
                self.phy_mmd_write(phy_addr, 2, 8, 0x03FF);  // Max RX delay

                // MMD 2, register 5: TX_CLK pad skew
                self.phy_mmd_write(phy_addr, 2, 5, 0x0000);  // No TX delay
            }
        }
        // Marvell PHY (common alternative)
        else if id1 == 0x0141 {
            // Marvell 88E1111 or similar
            // Page 2, register 21 for RGMII delays
            self.phy_write(phy_addr, 22, 2);       // Select page 2
            let val = self.phy_read(phy_addr, 21);
            self.phy_write(phy_addr, 21, val | 0x0030); // Enable RX/TX delays
            self.phy_write(phy_addr, 22, 0);       // Back to page 0
        }
        // Unknown PHY - no delay configuration
    }

    /// Write to PHY MMD (MDIO Manageable Device) register
    /// Used by newer PHYs like KSZ9031
    fn phy_mmd_write(&self, phy_addr: u8, devad: u8, reg: u16, val: u16) {
        // Set up address
        self.phy_write(phy_addr, 0x0D, devad as u16);        // MMD device address
        self.phy_write(phy_addr, 0x0E, reg);                  // MMD register address
        self.phy_write(phy_addr, 0x0D, 0x4000 | devad as u16); // Data, no post-inc
        self.phy_write(phy_addr, 0x0E, val);                  // Write data
    }

    /// Check if link is up
    pub fn link_up(&self) -> bool {
        if let Some(phy_addr) = self.phy_detect() {
            let status = self.phy_read(phy_addr, PHY_BMSR);
            status & BMSR_LINK_STATUS != 0
        } else {
            false
        }
    }
}

// Multicast/Broadcast MAC addresses
pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
pub const MULTICAST_FBC: [u8; 6] = [0x01, 0x00, 0x5E, 0x00, 0x00, 0xFB]; // 224.0.0.251

// =============================================================================
// FBC Protocol Support (Raw Ethernet)
// =============================================================================

use crate::fbc_protocol::{ETHERTYPE_FBC, FbcPacket};

impl GemEth {
    /// Send FBC packet (raw Ethernet frame)
    ///
    /// # Arguments
    /// * `dst_mac` - Destination MAC (unicast, broadcast, or multicast)
    /// * `packet` - FBC packet to send
    pub fn send_fbc(&mut self, dst_mac: [u8; 6], packet: &FbcPacket) -> bool {
        let mut frame = [0u8; 1536];

        // Ethernet header
        frame[0..6].copy_from_slice(&dst_mac);                    // Destination MAC
        frame[6..12].copy_from_slice(&self.mac);                 // Source MAC (our MAC)
        frame[12] = (ETHERTYPE_FBC >> 8) as u8;                  // EtherType high
        frame[13] = (ETHERTYPE_FBC & 0xFF) as u8;                // EtherType low

        // FBC packet (header + payload)
        let fbc_len = packet.serialize(&mut frame[14..]);

        if fbc_len == 0 {
            return false;
        }

        let total_len = 14 + fbc_len;

        // Send via GEM
        self.send(&frame[..total_len])
    }

    /// Receive FBC packet (non-blocking)
    ///
    /// Returns Some((packet, sender_mac)) if FBC frame received.
    /// The sender_mac can be used to unicast responses back.
    pub fn recv_fbc(&mut self) -> Option<(FbcPacket, [u8; 6])> {
        let mut frame = [0u8; 1536];

        // Try to receive frame
        let len = self.recv(&mut frame);
        if len == 0 {
            return None;
        }

        // Minimum frame: 14 (Ethernet) + 8 (FBC header) = 22 bytes
        if len < 22 {
            return None;
        }

        // Check EtherType (bytes 12-13)
        let ethertype = ((frame[12] as u16) << 8) | (frame[13] as u16);
        if ethertype != ETHERTYPE_FBC {
            return None;  // Not an FBC packet
        }

        // Extract sender MAC (bytes 6-11 of Ethernet frame)
        let sender_mac = [frame[6], frame[7], frame[8], frame[9], frame[10], frame[11]];

        // Parse FBC packet (skip Ethernet header)
        FbcPacket::parse(&frame[14..len]).map(|pkt| (pkt, sender_mac))
    }
}

/// Helper to build FBC packet with Ethernet frame
pub struct FbcFrameBuilder {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub packet: FbcPacket,
}

impl FbcFrameBuilder {
    pub fn new(dst_mac: [u8; 6], src_mac: [u8; 6], cmd: u8, seq: u16) -> Self {
        Self {
            dst_mac,
            src_mac,
            packet: FbcPacket::new(cmd, seq),
        }
    }

    pub fn with_payload(dst_mac: [u8; 6], src_mac: [u8; 6], cmd: u8, seq: u16, payload: &[u8]) -> Self {
        Self {
            dst_mac,
            src_mac,
            packet: FbcPacket::with_payload(cmd, seq, payload),
        }
    }

    /// Serialize to complete Ethernet frame
    pub fn serialize(&self, buf: &mut [u8]) -> usize {
        if buf.len() < 22 {
            return 0;
        }

        // Ethernet header
        buf[0..6].copy_from_slice(&self.dst_mac);
        buf[6..12].copy_from_slice(&self.src_mac);
        buf[12] = (ETHERTYPE_FBC >> 8) as u8;
        buf[13] = (ETHERTYPE_FBC & 0xFF) as u8;

        // FBC packet
        let fbc_len = self.packet.serialize(&mut buf[14..]);
        if fbc_len == 0 {
            return 0;
        }

        14 + fbc_len
    }
}
