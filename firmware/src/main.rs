//! FBC Semiconductor System - Main Entry Point
//!
//! Bare metal firmware for Zynq 7020.
//! Receives FBC programs over raw Ethernet, streams to FPGA, reports results.

#![no_std]
#![no_main]

use panic_halt as _;
use fbc_firmware::{
    FbcProtocolHandler, ControllerState, GemEth, NetConfig,
    FbcCtrl, VectorStatus, ErrorBram,
    Slcr, SdCard, Gpio, MioPin, Xadc, delay_ms,
    I2c, Spi, SpiMode, PowerSupplyManager,
    Eeprom, BimEeprom, EEPROM_ADDR, EEPROM_SIZE,
    Max11131, Bu2505, VicorController,
    AnalogMonitor, Gic,
    net::BROADCAST_MAC,
    fbc_protocol::{PendingVicor, PendingEeprom, PendingFastPins, ErrorLogEntry, error_log},
};

/// Helper to copy slice safely
trait CopyFromSlice {
    fn copy_from_slice(&mut self, src: &[u8]);
}
impl CopyFromSlice for [u8] {
    fn copy_from_slice(&mut self, src: &[u8]) {
        for (i, &byte) in src.iter().enumerate() {
            if i < self.len() { self[i] = byte; }
        }
    }
}

// =============================================================================
// Constants
// =============================================================================

/// Firmware version (major.minor as u16: 0x0100 = v1.0)
const FW_VERSION: u16 = 0x0100;

/// Heartbeat interval in loop iterations (roughly 100ms at typical loop rate)
const HEARTBEAT_INTERVAL: u32 = 100_000;

// =============================================================================
// Interrupt Handler (Cortex-A9 GIC)
// =============================================================================
// The actual IRQ dispatch is in hal/gic.rs (gic_irq_dispatch).
// It's called from boot.S _irq_handler via the GIC acknowledge/EOI flow.
// IRQ flags are set atomically in hal::gic::IRQ_FLAGS for the main loop.

// =============================================================================
// Entry Point
// =============================================================================

/// Entry point (called from startup assembly)
#[no_mangle]
pub extern "C" fn main() -> ! {
    // Ensure IRQs/FIQs are disabled during init (belt-and-suspenders with boot.S)
    unsafe { core::arch::asm!("cpsid if"); }

    // =========================================================================
    // PHASE 1: POWER SAFETY (do this FIRST before anything else)
    // =========================================================================

    let gpio = Gpio::new();

    // Initialize status LED first (MIO0 per schematic)
    gpio.init_status_led();
    gpio.set_status_led(true);  // LED ON = booting

    // Configure VICOR enable MIO pins as GPIO via SLCR mux
    // MIO pins 0 and 8 may default to UART/SPI functions in SLCR,
    // so we explicitly reconfigure them as GPIO before use.
    let slcr = Slcr::new();
    const VICOR_ENABLE_PINS: [u8; 6] = [0, 39, 47, 8, 38, 37];
    for &pin_num in &VICOR_ENABLE_PINS {
        slcr.configure_mio(pin_num, fbc_firmware::hal::slcr::mio::GPIO);
    }

    // Initialize VICOR enable pins as outputs, disabled (low)
    // Note: MIO 0 is also the status LED pin. Configuring it as a VICOR
    // enable output will turn off the LED. This is acceptable since the
    // LED is toggled again below at line 75 to indicate boot progress.
    for &pin_num in &VICOR_ENABLE_PINS {
        let pin = MioPin::new(pin_num);
        gpio.set_output(pin);
        gpio.write_pin(pin, false);
    }

    // Small delay for GPIO to settle
    delay_ms(10);
    gpio.toggle_status_led();  // Flicker

    // =========================================================================
    // PHASE 2: SYSTEM INITIALIZATION
    // =========================================================================

    let slcr = Slcr::new();
    let fbc = FbcCtrl::new();
    let status = VectorStatus::new();

    // Initialize XADC for internal monitoring (die temp, VCCINT, VCCAUX)
    let xadc = Xadc::new();
    xadc.init();

    // Check FPGA voltages are in safe range before proceeding
    // Check FPGA voltages are in safe range before proceeding
    let vccint = xadc.read_vccint_mv().unwrap_or(0);
    let vccaux = xadc.read_vccaux_mv().unwrap_or(0);
    let temp_mc = xadc.read_temperature_millicelsius().unwrap_or(99999); // Fail safe (trigger overtemp)

    // Safety checks — DISABLED during JTAG bring-up (XADC reads may be unreliable
    // before PS-XADC interface is fully initialized by FSBL).
    // TODO: Re-enable once XADC readings are verified on hardware.
    // if vccint < 900 || vccint > 1100 {
    //     hang_with_blink(2);  // 2 blinks = VCCINT out of range
    // }
    // if vccaux < 1700 || vccaux > 1900 {
    //     hang_with_blink(3);  // 3 blinks = VCCAUX out of range
    // }
    // if temp_mc > 85000 {  // 85°C max
    //     hang_with_blink(4);  // 4 blinks = overtemp
    // }

    gpio.toggle_status_led();  // Flicker - XADC done

    // SD Card (Flight Recorder) — DISABLED for JTAG bring-up
    // Root cause: SDHCI driver does byte writes to APB registers (e.g. Software Reset
    // at 0xE010002F). Without MMU, Cortex-A9 can't do byte-level writes to device
    // memory → alignment fault → Data Abort. Fix: SD driver needs 32-bit RMW for all
    // register accesses, or enable MMU first.
    let mut sd = SdCard::new();
    let sd_ok = false; // Skip sd.init() — causes Data Abort without MMU
    gpio.toggle_status_led();  // Flicker - SD skipped

    // =========================================================================
    // PHASE 2.5: PMBUS DISCOVERY
    // =========================================================================

    // Enable I2C clocks
    slcr.enable_i2c0();
    slcr.enable_i2c1();
    delay_ms(1);

    // Initialize I2C buses
    let i2c0 = I2c::i2c0();
    let i2c1 = I2c::i2c1();
    i2c0.init(100_000); // 100kHz standard mode
    i2c1.init(100_000);

    // Discover PMBus devices on both buses
    let mut psu_mgr = PowerSupplyManager::new_dual(&i2c0, &i2c1);
    let discovered = psu_mgr.discover();
    // discovered contains count of found power supplies
    // Can be used later for status reporting
    let _ = discovered; // Suppress unused warning for now
    gpio.toggle_status_led();  // Flicker - I2C/PMBus done

    // =========================================================================
    // PHASE 2.55: SPI / ADC / DAC / VICOR INITIALIZATION
    // =========================================================================

    // Initialize SPI0 for external ADC (MAX11131) and DAC (BU2505)
    slcr.enable_spi0();
    delay_ms(1);

    let spi0 = Spi::spi0();
    spi0.init(SpiMode::Mode0, 2);  // Mode 0, divisor 2 for ~50MHz

    // External ADC (MAX11131) - 16 channels, 12-bit, on SPI0/CS1
    let ext_adc = Max11131::new(&spi0);
    let _ = ext_adc.init();  // Ignore init errors for now

    // External DAC (BU2505) - 10 channels, 10-bit, on SPI0/CS0
    let dac = Bu2505::new(&spi0, 4096);  // 4.096V reference
    let _ = dac.init();

    // VICOR controller (DAC + GPIO for 6 cores)
    let mut vicor = VicorController::new(&dac, &gpio);
    let _ = vicor.init();  // Sets all cores to 0V and disabled

    // Analog Monitor (unified 32-channel interface: XADC + MAX11131)
    let analog_monitor = AnalogMonitor::new(&xadc, &ext_adc);
    gpio.toggle_status_led();  // Flicker - SPI/ADC/DAC done

    // =========================================================================
    // PHASE 2.6: EEPROM / BIM STATUS CHECK
    // =========================================================================

    // Try to read BIM EEPROM on I2C0 (24LC02 at address 0x50)
    let eeprom = Eeprom::new(&i2c0, EEPROM_ADDR);
    let mut eeprom_buf = [0u8; EEPROM_SIZE];

    // Check if EEPROM is present and read its contents
    let (has_bim, bim_programmed, bim_type, eeprom_serial) = match eeprom.read_all(&mut eeprom_buf) {
        Ok(()) => {
            // EEPROM responded - BIM is present
            let bim_data = BimEeprom::from_bytes(&eeprom_buf);
            if bim_data.is_programmed() {
                // EEPROM is programmed with valid magic
                (true, true, bim_data.bim_type, Some(bim_data.serial_number))
            } else {
                // EEPROM present but blank/unprogrammed
                (true, false, 0, None)
            }
        }
        Err(_) => {
            // EEPROM not responding - no BIM or I2C error
            (false, false, 0, None)
        }
    };

    // Read FPGA version to verify hardware is ready
    let version = fbc.get_version();
    if version == 0 || version == 0xFFFFFFFF {
        // FPGA not programmed or not responding
        hang_with_blink(1);  // 1 blink = FPGA error
    }

    // =========================================================================
    // PHASE 3: NETWORK INITIALIZATION
    // =========================================================================

    // Enable GEM0 peripheral clock (CRITICAL - was missing!)
    slcr.enable_gem0();
    // Configure GEM0 reference clocks (TX and RX)
    slcr.configure_gem0_clock();  // TX clock from PLL
    slcr.configure_gem0_rclk();   // RX clock from PHY
    delay_ms(1);

    // Reset PHY hardware before MDIO access (CRITICAL for PHY detection!)
    // The PHY won't respond to MDIO until hardware reset is complete.
    gpio.reset_phy();

    let config = NetConfig::from_dna();
    let mut eth = GemEth::new();
    eth.init(&config);

    // Initialize FBC protocol handler (needs identity for discovery responses)
    // Get device serial from DNA as fallback, use EEPROM serial if programmed
    let dna_serial = get_device_serial();
    let serial = eeprom_serial.unwrap_or(dna_serial);

    let mut handler = FbcProtocolHandler::new(config.mac, serial, FW_VERSION);
    handler.init();

    // Set BIM status for ANNOUNCE packets
    handler.set_bim_info(has_bim, bim_programmed, bim_type, eeprom_serial);

    // Send ANNOUNCE on boot (broadcast to all hosts)
    let announce = handler.build_announce();
    eth.send_fbc(BROADCAST_MAC, &announce);

    // =========================================================================
    // PHASE 4: INTERRUPT INITIALIZATION
    // =========================================================================

    // Initialize GIC and enable FBC interrupt (IRQ_F2P[0] = GIC ID 61)
    let gic = Gic::new();
    gic.init();

    // Enable IRQ in CPSR (was disabled in boot.S)
    unsafe { core::arch::asm!("cpsie i"); }

    // Boot complete - LED solid ON
    gpio.set_status_led(true);

    // Main loop
    let mut heartbeat_counter: u32 = 0;
    let mut log_index: u32 = 0;  // For SD card circular buffer
    let mut last_state = ControllerState::Idle;

    // Track sender MAC for unicast responses
    let mut last_sender_mac = BROADCAST_MAC;

    loop {
        // Poll for incoming FBC packets
        if let Some((packet, sender_mac)) = eth.recv_fbc() {
            last_sender_mac = sender_mac;

            // Process command and unicast response to sender
            if let Some(response) = handler.process(&packet) {
                eth.send_fbc(sender_mac, &response);
            }
        }

        // Update handler state
        handler.poll();

        // Handle pending Flight Recorder requests
        if let Some(log_req) = handler.take_pending_log_read() {
            let (status, data) = if sd_ok {
                match sd.read_block(log_req.sector, 1000) {
                    Ok(block) => (0, block),  // OK
                    Err(_) => (2, [0u8; 512]), // Read error
                }
            } else {
                (1, [0u8; 512])  // SD not present
            };
            let response = handler.build_log_read_response(log_req.sector, status, &data);
            eth.send_fbc(last_sender_mac, &response);
        }

        if handler.take_pending_log_info() {
            let response = handler.build_log_info_response(sd_ok);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle pending Analog Monitor requests
        if handler.take_pending_analog_read() {
            let mut readings = [(0u16, 0i32); 32];
            if let Ok(all) = analog_monitor.read_all() {
                for (i, r) in all.iter().enumerate() {
                    readings[i] = (r.raw, (r.value * 1000.0) as i32);
                }
            }
            let response = handler.build_analog_response(&readings);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle pending VICOR commands
        if let Some(vicor_cmd) = handler.take_pending_vicor() {
            match vicor_cmd {
                PendingVicor::StatusReq => {
                    let vicor_status = vicor.get_status();
                    // Convert to (enabled, voltage_mv, current_ma)
                    // Note: current reading would come from ADC, use 0 for now
                    let mut status_arr = [(false, 0u16, 0u16); 6];
                    for (i, (enabled, voltage)) in vicor_status.iter().enumerate() {
                        status_arr[i] = (*enabled, *voltage, 0);
                    }
                    let response = handler.build_vicor_status_response(&status_arr);
                    eth.send_fbc(last_sender_mac, &response);
                }
                PendingVicor::Enable { core_mask } => {
                    for core in 1..=6u8 {
                        if core_mask & (1 << (core - 1)) != 0 {
                            let _ = vicor.enable_core(core);
                        } else {
                            let _ = vicor.disable_core(core);
                        }
                    }
                }
                PendingVicor::SetVoltage { core, mv } => {
                    let _ = vicor.set_core_voltage(core, mv);
                }
                PendingVicor::EmergencyStop => {
                    vicor.disable_all();
                    psu_mgr.disable_all();  // Also kill PMBus/LCPS rails
                }
                PendingVicor::PowerSequenceOn { voltages_mv } => {
                    let _ = vicor.power_on_sequence(&voltages_mv);
                }
                PendingVicor::PowerSequenceOff => {
                    let _ = vicor.power_off_sequence();
                }
            }
        }

        // Handle pending PMBus commands
        if let Some(pmbus_cmd) = handler.take_pending_pmbus() {
            // Use the power supply manager to enable/disable by I2C address
            if pmbus_cmd.enable {
                let _ = psu_mgr.enable_by_addr(pmbus_cmd.addr);
            } else {
                let _ = psu_mgr.disable_by_addr(pmbus_cmd.addr);
            }
        }

        // Handle pending EEPROM commands
        if let Some(eeprom_cmd) = handler.take_pending_eeprom() {
            match eeprom_cmd {
                PendingEeprom::Read { offset, len } => {
                    let mut data = [0u8; 64];
                    let read_len = (len as usize).min(64);
                    let success = eeprom.read(offset, &mut data[..read_len]).is_ok();
                    if success {
                        let response = handler.build_eeprom_read_response(offset, &data[..read_len]);
                        eth.send_fbc(last_sender_mac, &response);
                    } else {
                        let response = handler.build_eeprom_read_response(offset, &[]);
                        eth.send_fbc(last_sender_mac, &response);
                    }
                }
                PendingEeprom::Write { offset, len, data } => {
                    let write_len = (len as usize).min(64);
                    let success = eeprom.write(offset, &data[..write_len]).is_ok();
                    let response = handler.build_eeprom_write_ack(success);
                    eth.send_fbc(last_sender_mac, &response);
                }
            }
        }

        // Handle pending Fast Pins commands
        if let Some(fastpins_cmd) = handler.take_pending_fastpins() {
            match fastpins_cmd {
                PendingFastPins::Read => {
                    // Read from AXI registers at 0x20, 0x24, 0x28
                    let din = fbc.read_fast_din();
                    let dout = fbc.read_fast_dout();
                    let oen = fbc.read_fast_oen();
                    let response = handler.build_fastpins_response(din, dout, oen);
                    eth.send_fbc(last_sender_mac, &response);
                }
                PendingFastPins::Write { dout, oen } => {
                    fbc.write_fast_dout(dout);
                    fbc.write_fast_oen(oen);
                }
            }
        }

        // Handle pending Error Log requests
        if let Some(error_log_req) = handler.take_pending_error_log() {
            let error_bram = ErrorBram::new();
            let total_errors = status.get_error_count();
            let mut entries = [ErrorLogEntry {
                pattern: [0; 4],
                vector: 0,
                cycle_lo: 0,
                cycle_hi: 0,
            }; 8];

            let count = (error_log_req.count as usize).min(8);
            for i in 0..count {
                let index = error_log_req.start_index as usize + i;
                if index >= total_errors as usize {
                    break;
                }
                error_bram.set_read_index(index as u32);
                let pattern = error_bram.read_pattern();
                let vector = error_bram.read_vector();
                let cycle = error_bram.read_cycle();
                entries[i] = ErrorLogEntry {
                    pattern,
                    vector,
                    cycle_lo: cycle as u32,
                    cycle_hi: (cycle >> 32) as u32,
                };
            }

            let response = handler.build_error_log_response(total_errors, &entries[..count]);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Check for state transitions
        let current_state = handler.state();
        if current_state != last_state {
            match current_state {
                ControllerState::Done => {
                    // Test completed successfully
                    let _cycles = status.get_cycle_count();
                    let _errors = status.get_error_count();
                    // State will be reported in next heartbeat or STATUS_RSP
                }
                ControllerState::Error => {
                    // Error occurred - send ERROR packet
                    let error_pkt = handler.build_error(
                        0,  // error_type: vector mismatch
                        status.get_cycle_count() as u32,
                        status.get_error_count(),
                    );
                    eth.send_fbc(last_sender_mac, &error_pkt);
                }
                _ => {}
            }
            last_state = current_state;
        }

        // Send periodic heartbeat during test
        if current_state == ControllerState::Running {
            heartbeat_counter += 1;
            if heartbeat_counter >= HEARTBEAT_INTERVAL {
                heartbeat_counter = 0;

                // Build and send heartbeat
                handler.log_index = log_index;  // Use current value
                let heartbeat = handler.build_heartbeat();
                eth.send_fbc(last_sender_mac, &heartbeat);

                // Flight Recorder: Log heartbeat to SD (Sectors 1001-2000 circular buffer)
                if sd_ok {
                    let log_sector = 1001 + (log_index % 1000);
                    let hb_bytes: &[u8] = unsafe {
                        core::slice::from_raw_parts(
                            &heartbeat as *const _ as *const u8,
                            core::mem::size_of_val(&heartbeat)
                        )
                    };
                    let mut sector_buf = [0u8; 512];
                    sector_buf[..hb_bytes.len().min(512)].copy_from_slice(
                        &hb_bytes[..hb_bytes.len().min(512)]
                    );
                    let _ = sd.write_block(log_sector, &sector_buf);
                }

                // Increment AFTER writing so first heartbeat uses sector 1001
                log_index += 1;
            }
        } else {
            heartbeat_counter = 0;
        }

        // Yield to prevent hogging CPU
        core::hint::spin_loop();
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Get device serial number from DNA
fn get_device_serial() -> u32 {
    use fbc_firmware::hal::read_device_dna;
    let dna = read_device_dna();
    // Use lower 32 bits of DNA as serial
    dna as u32
}

/// Hang with LED blink pattern for debugging
/// Pattern indicates error type:
/// - 1 blink = FPGA error
/// - 2 blinks = VCCINT out of range
/// - 3 blinks = VCCAUX out of range
/// - 4 blinks = Overtemp
fn hang_with_blink(pattern: u32) -> ! {
    let gpio = Gpio::new();
    gpio.init_status_led();

    loop {
        // Blink N times
        for _ in 0..pattern {
            gpio.set_status_led(true);
            delay_ms(200);
            gpio.set_status_led(false);
            delay_ms(200);
        }
        // Long pause between patterns
        delay_ms(1000);
    }
}

// =============================================================================
// Startup Code (ARM Assembly)
// =============================================================================

// Include ARM assembly startup code
// This handles: stack setup, BSS clear, FPU enable, then calls main()
core::arch::global_asm!(include_str!("boot.S"));
