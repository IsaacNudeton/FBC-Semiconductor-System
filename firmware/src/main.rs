//! FBC Semiconductor System - Main Entry Point
//!
//! Bare metal firmware for Zynq 7020.
//! Receives FBC programs over raw Ethernet, streams to FPGA, reports results.

#![no_std]
#![no_main]

use panic_halt as _;
use fbc_firmware::{
    uart_println,
    FbcProtocolHandler, ControllerState, GemEth, NetConfig,
    FbcCtrl, VectorStatus, ErrorBram, ClkCtrl, VecClockFreq,
    Slcr, SdCard, Gpio, MioPin, Xadc, delay_ms,
    I2c, Spi, SpiMode, PowerSupplyManager,
    Eeprom, BimEeprom, EEPROM_ADDR, EEPROM_SIZE,
    Max11131, Bu2505, VicorController,
    AnalogMonitor, Gic,
    BoardConfig,
    net::BROADCAST_MAC,
    fbc_protocol::{FbcPacket, PendingVicor, PendingPmbus, PendingEeprom, PendingFastPins, PendingBoardConfig, ErrorLogEntry},
};
use fbc_firmware::hal::thermal::{Thermal, output_to_heater, output_to_fan};


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
    // PHASE 0: OCM REMAP + UART (absolute first — before ANY OCM or debug output)
    // =========================================================================
    // Map all 4 OCM banks to HIGH address range (0xFFFC0000-0xFFFFFFFF).
    // GEM0 DMA descriptors live there. Without this, writes to 0xFFFC0000 abort.
    // arm_loader.py did this for JTAG boots; SD card FSBL sets 0x18 (only bank 3).
    // CRITICAL: SLCR must be unlocked before writing OCM_CFG!
    unsafe {
        core::ptr::write_volatile(0xF800_0008 as *mut u32, 0x0000_DF0D); // Unlock SLCR
        core::ptr::write_volatile(0xF800_0910 as *mut u32, 0x0000_000F); // OCM all HIGH
        core::ptr::write_volatile(0xF800_0004 as *mut u32, 0x0000_767B); // Re-lock SLCR
    }

    // UART0 for boot console — FSBL already configured the baud rate and MIO pins,
    // so we can just write to the TX FIFO immediately without full init.
    // Full init would reset the FIFO and reconfigure, which is fine too.
    let uart = fbc_firmware::hal::Uart::uart0();
    uart.init_default(); // 115200 8N1

    uart_println!("");
    uart_println!("========================================");
    uart_println!("  FBC Semiconductor System v1.0");
    uart_println!("  Bare-metal Rust on Zynq 7020");
    uart_println!("========================================");
    uart_println!("[BOOT] OCM remapped to HIGH (0x0F)");

    // =========================================================================
    // PHASE 1: POWER SAFETY (do this FIRST before anything else)
    // =========================================================================
    uart_println!("[BOOT] Phase 1: Power safety...");

    let gpio = Gpio::new();

    // No user-visible LEDs on this board (only 3.3V power + DONE, both hardware-driven)
    let slcr = Slcr::new();

    // Configure VICOR enable MIO pins as GPIO via SLCR mux
    // MIO pins 37-39,47 may default to UART/SPI functions in SLCR,
    // so we explicitly reconfigure them as GPIO before use.
    // Core mapping (from schematic + Sonoma AWK): Core1=MIO0, Core2=MIO39,
    // Core3=MIO47, Core4=MIO8, Core5=MIO38, Core6=MIO37
    // NOTE: MIO11 is PHY_RESET - must be GPIO for Ethernet to work!
    const VICOR_ENABLE_PINS: [u8; 6] = [0, 39, 47, 8, 38, 37];
    for &pin_num in &VICOR_ENABLE_PINS {
        slcr.configure_mio(pin_num, fbc_firmware::hal::slcr::mio::GPIO);
    }
    // Also configure PHY_RESET (MIO11) as GPIO
    slcr.configure_mio(11, fbc_firmware::hal::slcr::mio::GPIO);

    // Initialize VICOR enable pins as outputs, disabled (low)
    for &pin_num in &VICOR_ENABLE_PINS {
        let pin = MioPin::new(pin_num);
        gpio.set_output(pin);
        gpio.write_pin(pin, false);
    }

    // Small delay for GPIO to settle
    delay_ms(10);
    uart_println!("[BOOT] VICOR pins safe (all disabled)");

    // =========================================================================
    // PHASE 2: SYSTEM INITIALIZATION
    // =========================================================================
    uart_println!("[BOOT] Phase 2: System init...");

    let slcr = Slcr::new();
    let fbc = FbcCtrl::new();
    let status = VectorStatus::new();
    let clk_ctrl = ClkCtrl::new();

    // Enable PCAP clock (required for PS-XADC interface to function)
    slcr.enable_pcap();
    delay_ms(1);

    // Initialize XADC for internal monitoring (die temp, VCCINT, VCCAUX)
    let xadc = Xadc::new();
    xadc.init();

    // Read XADC values — don't hang on failure, always boot so we can debug via CLI.
    // Safety status reported in STATUS_RSP telemetry, not enforced at boot.
    let _vccint = xadc.read_vccint_mv().unwrap_or(0);
    let _vccaux = xadc.read_vccaux_mv().unwrap_or(0);
    let _temp_mc = xadc.read_temperature_millicelsius().unwrap_or(0);
    // XADC init done — values are read live in handle_status_req()
    uart_println!("[BOOT] XADC initialized");

    // SD Card (Flight Recorder)
    // SKIP SD init for now — hangs when FSBL left SDHCI in post-boot state.
    // The SD card is used for flight recorder only; not critical for burn-in.
    // TODO: Fix SDHCI re-init after FSBL, or add hardware timeout.
    let sd = SdCard::new();
    let sd_init_ok = false; // sd.init(&slcr).is_ok();

    // Flight Recorder: corruption-resistant log with dual headers + CRC32
    let mut recorder = fbc_firmware::flight_recorder::FlightRecorder::new();
    if sd_init_ok {
        let _ = recorder.init(&sd);
    }

    if sd_init_ok {
        uart_println!("[BOOT] SD card initialized");
    } else {
        uart_println!("[BOOT] SD card not present or failed");
    }

    // =========================================================================
    // PHASE 2.5: PMBUS DISCOVERY
    // =========================================================================
    uart_println!("[BOOT] Phase 2.5: PMBus discovery...");

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
    // I2C/PMBus init done

    uart_println!("[BOOT] I2C/PMBus init done");

    // =========================================================================
    // PHASE 2.55: SPI / ADC / DAC / VICOR INITIALIZATION
    // =========================================================================
    uart_println!("[BOOT] Phase 2.55: SPI/ADC/DAC/VICOR...");

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
    let mut analog_monitor = AnalogMonitor::new(&xadc, &ext_adc);
    // SPI/ADC/DAC init done

    uart_println!("[BOOT] SPI/ADC/DAC/VICOR init done");

    // =========================================================================
    // PHASE 2.6: EEPROM / BIM STATUS CHECK
    // =========================================================================
    uart_println!("[BOOT] Phase 2.6: EEPROM/BIM check...");

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

    // Build BoardConfig — merges EEPROM defaults with runtime overrides
    let mut board_config = if has_bim && bim_programmed {
        let bim_data = BimEeprom::from_bytes(&eeprom_buf);
        let cfg = BoardConfig::from_eeprom(bim_data);
        let serial = { bim_data.serial_number };
        uart_println!("[BOOT] BIM detected: {} (serial=0x{:08X})",
            cfg.bim_type().name(), serial);
        uart_println!("[BOOT]   Rail limits loaded from EEPROM");
        uart_println!("[BOOT]   Calibration offsets loaded from EEPROM");
        cfg
    } else if has_bim {
        uart_println!("[BOOT] BIM detected but NOT programmed — using safe defaults");
        BoardConfig::no_eeprom()
    } else {
        uart_println!("[BOOT] No BIM/EEPROM present — using safe defaults");
        BoardConfig::no_eeprom()
    };

    // Initialize thermal controller (ONETWO crystallization feedforward)
    // Uses board_config temp setpoint as target. Output logged but not routed
    // to physical heater/fan GPIO yet (PWM pins not identified on this board).
    let mut thermal = Thermal::new();
    let setpoint_dc = board_config.temp_setpoint_dc();
    thermal.set_target((setpoint_dc as i32) * 100); // deci-Celsius → milliCelsius
    uart_println!("[BOOT] Thermal controller: target={}°C", setpoint_dc / 10);

    // Read FPGA version to verify hardware is ready
    let version = fbc.get_version();
    if version == 0 || version == 0xFFFFFFFF {
        // FPGA not programmed or not responding
        uart_println!("[BOOT] FATAL: FPGA not responding!");
        hang_with_blink(1);  // 1 blink = FPGA error
    }
    uart_println!("[BOOT] FPGA version: 0x{:08X}", version);

    // =========================================================================
    // PHASE 3: NETWORK INITIALIZATION
    // =========================================================================
    uart_println!("[BOOT] Phase 3: Network init...");

    // Enable GEM0 peripheral clock (CRITICAL - was missing!)
    slcr.enable_gem0();
    // Configure GEM0 reference clocks (TX and RX)
    slcr.configure_gem0_clock();  // TX clock from PLL
    slcr.configure_gem0_rclk();   // RX clock from PHY
    delay_ms(1);

    // MDIO pins (MIO 52-53) are set by ps7_init.tcl / arm_loader.py as
    // L3_SEL=4 (0x1480) which IS GEM0 MDIO (Zynq TRM Table B.5).
    // Do NOT override — L0_SEL=1 is wrong for these pins and breaks MDIO.

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

    uart_println!("[BOOT] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        config.mac[0], config.mac[1], config.mac[2],
        config.mac[3], config.mac[4], config.mac[5]);

    // Initialize DDR double-buffer + SD pattern directory
    let mut ddr_buf = fbc_firmware::DdrBuffer::new();
    let mut pattern_dir = fbc_firmware::PatternDirectory::new();
    let bim_serial_for_slots = eeprom_serial.unwrap_or(0);

    // Try to load pattern directory from SD
    if sd_init_ok {
        if let Ok(count) = pattern_dir.load_from_sd(&sd) {
            uart_println!("[BOOT] SD pattern directory: {} patterns loaded", count);
        } else {
            uart_println!("[BOOT] SD pattern directory: load failed (empty or corrupt)");
        }
    } else {
        uart_println!("[BOOT] SD not available — patterns must be uploaded via Ethernet");
    }

    // Initialize test plan executor (autonomous burn-in)
    let mut plan_executor = fbc_firmware::PlanExecutor::new();
    let mut last_checkpoint_ms: u32 = 0;
    let mut pending_sd_load: Option<fbc_firmware::SdLoadState> = None; // Chunked SD→DDR load in progress

    // FBC loader for plan-driven vector loading (reused across steps)
    let mut plan_loader = fbc_firmware::FbcLoader::new();
    plan_loader.init();

    // Check for DDR checkpoint — resume plan after warm reset
    if let Some(cp) = plan_executor.read_checkpoint_from_ddr() {
        if cp.bim_serial == bim_serial_for_slots && cp.elapsed_secs < cp.total_duration_secs {
            uart_println!("[BOOT] Plan checkpoint found: step {}/{}, {}s/{}s elapsed, {} loops",
                cp.current_step, cp.num_steps, cp.elapsed_secs, cp.total_duration_secs, cp.plan_loops);
            // Plan definition is in the executor already from set_plan(), but after reset
            // the RAM is gone. We need the plan steps re-sent, OR we persist them too.
            // For now: mark as resumable, host will re-send plan + RUN_PLAN on reconnect.
            // The checkpoint tells the host where we were so it can adjust.
            uart_println!("[BOOT] Plan resumable — host must re-send plan definition");
        } else {
            // BIM changed or plan was already done — clear stale checkpoint
            plan_executor.clear_checkpoint();
            uart_println!("[BOOT] Stale plan checkpoint cleared");
        }
    }

    // Send ANNOUNCE on boot (broadcast 3x so listeners don't miss it)
    for i in 0..3 {
        let announce = handler.build_announce();
        eth.send_fbc(BROADCAST_MAC, &announce);
        if i < 2 { delay_ms(100); }
    }
    uart_println!("[BOOT] ANNOUNCE sent (3x broadcast)");

    // =========================================================================
    // PHASE 4: INTERRUPT INITIALIZATION
    // =========================================================================

    // Initialize GIC and enable FBC interrupt (IRQ_F2P[0] = GIC ID 61)
    let gic = Gic::new();
    gic.init();

    // Enable IRQ in CPSR (was disabled in boot.S)
    unsafe { core::arch::asm!("cpsie i"); }

    uart_println!("[BOOT] Init complete — entering main loop");
    uart_println!("========================================");

    // Main loop
    let mut heartbeat_counter: u32 = 0;
    let mut last_state = ControllerState::Idle;

    // Idle heartbeat: broadcast ANNOUNCE periodically so monitor can see us
    const IDLE_HEARTBEAT_INTERVAL: u32 = 5_000_000; // ~5 seconds at typical loop rate
    let mut idle_heartbeat_counter: u32 = 0;

    // Safety monitor: check temp/current every N iterations (~500ms)
    const SAFETY_CHECK_INTERVAL: u32 = 500_000;
    let mut safety_counter: u32 = 0;
    let mut safety_tripped: bool = false; // Latched — stays true until host sends RESET

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

        // Handle RESET — clear safety latch so board can resume operation
        if handler.take_pending_reset() {
            if safety_tripped {
                uart_println!("[SAFETY] Reset received — clearing safety latch");
                safety_tripped = false;
                safety_counter = 0;
            }
        }

        // Handle pending Flight Recorder requests
        if let Some(log_req) = handler.take_pending_log_read() {
            let (status, data) = if recorder.sd_ok {
                match recorder.read_sector(&sd, log_req.sector) {
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
            let response = handler.build_log_info_response(
                recorder.sd_ok,
                recorder.health as u8,
                recorder.data_start(),
                recorder.capacity(),
                recorder.write_index(),
                recorder.total_entries(),
            );
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle SD_FORMAT request
        if handler.pending_sd_format {
            handler.pending_sd_format = false;
            let status = if !sd_init_ok {
                2u8 // SD not initialized
            } else {
                match recorder.format(&sd) {
                    Ok(()) => 0u8,
                    Err(_) => 1u8,
                }
            };
            let response = handler.build_sd_format_ack(status);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle SD_REPAIR request
        if handler.pending_sd_repair {
            handler.pending_sd_repair = false;
            if !sd_init_ok {
                let response = handler.build_sd_repair_ack(1, 0); // SD not initialized
                eth.send_fbc(last_sender_mac, &response);
            } else {
                let health = recorder.repair(&sd);
                let status = if recorder.sd_ok { 0u8 } else { 1u8 };
                let response = handler.build_sd_repair_ack(status, health as u8);
                eth.send_fbc(last_sender_mac, &response);
            }
        }

        // Handle pending Analog Monitor requests (with EEPROM/override calibration)
        if handler.take_pending_analog_read() {
            let mut readings = [(0u16, 0i32); 32];
            if let Ok(all) = analog_monitor.read_all() {
                for (i, r) in all.iter().enumerate() {
                    let raw_val = (r.value * 1000.0) as i32;
                    // Apply per-channel calibration offset from EEPROM + overrides
                    let calibrated = board_config.calibrate_voltage(i, raw_val);
                    readings[i] = (r.raw, calibrated);
                }
            }
            let response = handler.build_analog_response(&readings);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle pending VICOR commands (with rail limit enforcement)
        if let Some(vicor_cmd) = handler.take_pending_vicor() {
            match vicor_cmd {
                PendingVicor::StatusReq => {
                    let vicor_status = vicor.get_status();
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
                    // Enforce hardware limits — EEPROM/override limits checked
                    match board_config.check_vicor_voltage(core, mv) {
                        Ok(safe_mv) => {
                            let _ = vicor.set_core_voltage(core, safe_mv);
                        }
                        Err(limit) => {
                            uart_println!("[SAFETY] VICOR core {} voltage {}mV REJECTED (max={}mV)",
                                core, mv, limit);
                            // Don't set — voltage exceeds hardware limit
                        }
                    }
                }
                PendingVicor::EmergencyStop => {
                    vicor.disable_all();
                    psu_mgr.disable_all();
                }
                PendingVicor::PowerSequenceOn { voltages_mv } => {
                    // Check all voltages before powering on
                    let mut safe = true;
                    for (i, &mv) in voltages_mv.iter().enumerate() {
                        if mv != 0 {
                            if let Err(limit) = board_config.check_vicor_voltage((i + 1) as u8, mv) {
                                uart_println!("[SAFETY] PowerOn core {} voltage {}mV REJECTED (max={}mV)",
                                    i + 1, mv, limit);
                                safe = false;
                            }
                        }
                    }
                    if safe {
                        let _ = vicor.power_on_sequence(&voltages_mv);
                    } else {
                        uart_println!("[SAFETY] Power sequence ABORTED — voltage limit violation");
                    }
                }
                PendingVicor::PowerSequenceOff => {
                    let _ = vicor.power_off_sequence();
                }
            }
        }

        // Handle pending PMBus status request
        if handler.take_pending_pmbus_status() {
            psu_mgr.update_telemetry();
            let supplies = psu_mgr.all();
            let mut status_arr = [(0u8, 0u8, false, 0u32, 0i32); 16];
            let count = supplies.len().min(16);
            for (i, s) in supplies.iter().enumerate().take(16) {
                status_arr[i] = (s.address, s.bus, s.output_on, s.vout_mv, s.iout_ma);
            }
            let response = handler.build_pmbus_status_response(&status_arr[..count]);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle pending PMBus commands
        if let Some(pmbus_cmd) = handler.take_pending_pmbus() {
            match pmbus_cmd {
                PendingPmbus::Enable { addr, enable } => {
                    if enable {
                        let _ = psu_mgr.enable_by_addr(addr);
                    } else {
                        let _ = psu_mgr.disable_by_addr(addr);
                    }
                }
                PendingPmbus::SetVoltage { channel, voltage_mv } => {
                    // Safety check against EEPROM limits
                    match board_config.check_pmbus_voltage(channel, voltage_mv) {
                        Ok(()) => {
                            let _ = psu_mgr.set_voltage_by_channel(channel, voltage_mv as u32);
                            uart_println!("[PMBus] ch{} set to {}mV", channel, voltage_mv);
                        }
                        Err(violation) => {
                            uart_println!("[SAFETY] PMBus ch{} voltage {}mV REJECTED: {:?}",
                                channel, voltage_mv, violation);
                        }
                    }
                }
            }
        }

        // Handle pending EEPROM commands
        // Skip I2C access if no BIM detected at boot (prevents crash on boards without EEPROM)
        if let Some(eeprom_cmd) = handler.take_pending_eeprom() {
            if !has_bim {
                // No EEPROM present — return empty data / failure without touching I2C
                match eeprom_cmd {
                    PendingEeprom::Read { offset, .. } => {
                        let response = handler.build_eeprom_read_response(offset, &[]);
                        eth.send_fbc(last_sender_mac, &response);
                    }
                    PendingEeprom::Write { .. } | PendingEeprom::WriteBim { .. } => {
                        let response = handler.build_eeprom_write_ack(false);
                        eth.send_fbc(last_sender_mac, &response);
                    }
                }
            } else {
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
                    PendingEeprom::WriteBim { data } => {
                        // Full 256-byte BIM programming — write, verify, reload BoardConfig
                        let success = eeprom.write_all(&data).is_ok();
                        if success {
                            // Re-read to verify and reload BoardConfig with new data
                            let mut verify_buf = [0u8; EEPROM_SIZE];
                            if eeprom.read_all(&mut verify_buf).is_ok() {
                                let bim_data = BimEeprom::from_bytes(&verify_buf);
                                if bim_data.is_programmed() {
                                    board_config = BoardConfig::from_eeprom(bim_data);
                                    // Update protocol handler's BIM info
                                    let bim_type = { bim_data.bim_type };
                                    let serial = { bim_data.serial_number };
                                    handler.set_bim_info(true, true, bim_type, Some(serial));
                                    uart_println!("[BIM] Programmed: {} serial=0x{:08X} — BoardConfig reloaded",
                                        bim_data.get_bim_type().name(), serial);
                                }
                            }
                        }
                        let response = handler.build_eeprom_write_ack(success);
                        eth.send_fbc(last_sender_mac, &response);
                    }
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

        // Handle pending Board Config override commands
        if let Some(cfg_cmd) = handler.take_pending_board_config() {
            match cfg_cmd {
                PendingBoardConfig::SetOverride { field_id, value } => {
                    let overrides = board_config.overrides_mut();
                    match field_id {
                        // Rail max voltage (0x01-0x08 → rail 0-7)
                        0x01..=0x08 => {
                            let rail = (field_id - 0x01) as usize;
                            overrides.rail_max_voltage_mv[rail] = value as u16;
                        }
                        // Rail min voltage (0x11-0x18 → rail 0-7)
                        0x11..=0x18 => {
                            let rail = (field_id - 0x11) as usize;
                            overrides.rail_min_voltage_mv[rail] = value as u16;
                        }
                        // Rail max current (0x21-0x28 → rail 0-7)
                        0x21..=0x28 => {
                            let rail = (field_id - 0x21) as usize;
                            overrides.rail_max_current_ma[rail] = value as u16;
                        }
                        // Voltage cal offset (0x40-0x4F → channel 0-15)
                        0x40..=0x4F => {
                            let ch = (field_id - 0x40) as usize;
                            overrides.voltage_cal[ch] = value;
                            overrides.voltage_cal_set |= 1 << ch;
                        }
                        // Current cal offset (0x50-0x5F → channel 0-15)
                        0x50..=0x5F => {
                            let ch = (field_id - 0x50) as usize;
                            overrides.current_cal[ch] = value;
                            overrides.current_cal_set |= 1 << ch;
                        }
                        // Temperature setpoint (0x80)
                        0x80 => {
                            overrides.temp_setpoint_dc = value;
                            overrides.temp_setpoint_set = true;
                            // Update thermal controller target
                            thermal.set_target((value as i32) * 100); // deci-C → milli-C
                        }
                        _ => {} // Unknown field — ignore
                    }
                }
                PendingBoardConfig::ClearAll => {
                    board_config.clear_overrides();
                    uart_println!("[CONFIG] All overrides cleared — EEPROM defaults restored");
                }
                PendingBoardConfig::GetEffective => {
                    // Build effective config response
                    let mut rail_limits = [(0u16, 0u16, 0u16); 8];
                    for i in 0..8 {
                        let eff = board_config.effective_rail(i);
                        rail_limits[i] = (eff.max_voltage_mv, eff.min_voltage_mv, eff.max_current_ma);
                    }
                    let mut vcal = [0i16; 16];
                    let mut ical = [0i16; 16];
                    for i in 0..16 {
                        vcal[i] = board_config.voltage_cal_offset(i);
                        ical[i] = board_config.current_cal_offset(i);
                    }
                    let temp = board_config.temp_setpoint_dc();
                    let response = handler.build_effective_config_response(
                        &rail_limits, &vcal, &ical, temp,
                    );
                    eth.send_fbc(last_sender_mac, &response);
                }
            }
        }

        // Handle pending Firmware Info request
        if handler.take_pending_fw_info() {
            let response = handler.build_fw_info_rsp(recorder.sd_ok);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle pending Firmware Update BEGIN
        if let Some(begin) = handler.pending_fw_begin.take() {
            if !sd_init_ok {
                // No SD card — reject
                let response = handler.build_fw_begin_ack(1); // 1 = SD not present
                eth.send_fbc(last_sender_mac, &response);
            } else {
                // Accept update
                handler.start_fw_update(begin.total_size, begin.checksum);
                let response = handler.build_fw_begin_ack(0); // 0 = OK
                eth.send_fbc(last_sender_mac, &response);
            }
        }

        // Handle pending Firmware Update CHUNK
        if let Some(chunk) = handler.pending_fw_chunk.take() {
            let chunk_size = chunk.size as u32;
            if sd_init_ok {
                // Write chunk data to SD card (firmware area starts at sector 2048)
                // Each 512-byte sector holds part of the firmware image
                let start_sector = 2048 + (chunk.offset / 512);
                let bytes_to_write = (chunk_size as usize).min(chunk.data.len());
                // Write in 512-byte blocks
                let mut written = 0usize;
                let mut status = 0u8;
                while written < bytes_to_write {
                    let block_offset = written;
                    let block_len = (bytes_to_write - written).min(512);
                    let mut sector_buf = [0u8; 512];
                    sector_buf[..block_len].copy_from_slice(&chunk.data[block_offset..block_offset + block_len]);
                    let sector = start_sector + (written as u32 / 512);
                    if sd.write_block(sector, &sector_buf).is_err() {
                        status = 2; // Write error
                        break;
                    }
                    written += 512;
                }
                // Simple CRC: XOR all data words
                let mut crc = 0u32;
                for i in (0..bytes_to_write).step_by(4) {
                    if i + 4 <= chunk.data.len() {
                        let word = u32::from_le_bytes([
                            chunk.data[i], chunk.data[i+1],
                            chunk.data[i+2], chunk.data[i+3],
                        ]);
                        crc ^= word;
                    }
                }
                handler.process_fw_chunk(chunk_size, crc);
                let response = handler.build_fw_chunk_ack(chunk.offset, status);
                eth.send_fbc(last_sender_mac, &response);
            } else {
                let response = handler.build_fw_chunk_ack(chunk.offset, 1); // SD error
                eth.send_fbc(last_sender_mac, &response);
            }
        }

        // Handle pending Firmware Update COMMIT
        if handler.pending_fw_commit {
            handler.pending_fw_commit = false;
            // Verify received size matches expected
            let (received, total) = handler.get_fw_update_progress();
            let status = if received == total { 0 } else { 1 }; // 0=OK, 1=size mismatch
            let response = handler.build_fw_commit_ack(status);
            eth.send_fbc(last_sender_mac, &response);
        }

        // =====================================================================
        // PATTERN UPLOAD — write .fbc data to SD card pattern storage
        // =====================================================================

        // Handle pending pattern upload chunk (writes to SD, not DDR)
        if let Some(upload) = handler.pending_slot_upload.take() {
            if sd_init_ok {
                let pattern_id = upload.slot_id as usize; // reusing field — pattern ID

                // Compute base sector: use existing entry if valid, else allocate sequentially
                let base_sector = if pattern_dir.entries.get(pattern_id)
                    .map(|e| e.is_valid()).unwrap_or(false)
                {
                    fbc_firmware::SD_PATTERN_DATA_SECTOR
                        + pattern_dir.entries[pattern_id].start_sector
                } else {
                    // New pattern: allocate after all existing data
                    let mut next_sector = 0u32;
                    for e in pattern_dir.entries.iter() {
                        if e.is_valid() {
                            let end = e.start_sector + e.sector_count();
                            if end > next_sector { next_sector = end; }
                        }
                    }
                    fbc_firmware::SD_PATTERN_DATA_SECTOR + next_sector
                };

                // Write chunk data to SD
                let sector_offset = upload.offset / 512;
                let chunk_data = &upload.data[..upload.chunk_size as usize];
                let chunk_sectors = (chunk_data.len() + 511) / 512;
                for i in 0..chunk_sectors {
                    let off = i * 512;
                    let len = (chunk_data.len() - off).min(512);
                    let mut sector_buf = [0u8; 512];
                    sector_buf[..len].copy_from_slice(&chunk_data[off..off + len]);
                    let _ = sd.write_block(base_sector + sector_offset + i as u32, &sector_buf);
                }

                // On last chunk: finalize — write directory entry to SD
                if upload.offset + upload.chunk_size as u32 >= upload.total_size {
                    let relative_start = base_sector - fbc_firmware::SD_PATTERN_DATA_SECTOR;
                    let entry = fbc_firmware::PatternEntry {
                        start_sector: relative_start,
                        size_bytes: upload.total_size,
                        num_vectors: 0, // Updated when pattern is first loaded
                        vec_clock_hz: 0,
                    };

                    // Write directory entry to SD (sector 8 + pattern_id/32, offset within sector)
                    if pattern_id < fbc_firmware::MAX_PATTERNS {
                        pattern_dir.entries[pattern_id] = entry;
                        if pattern_id as u16 >= pattern_dir.count {
                            pattern_dir.count = pattern_id as u16 + 1;
                        }

                        // Persist to SD: each sector holds 32 entries × 16 bytes
                        let dir_sector = fbc_firmware::ddr_slots::SD_DIRECTORY_SECTOR
                            + (pattern_id / 32) as u32;
                        // Read existing sector, update our entry, write back
                        if let Ok(mut block) = sd.read_block(dir_sector, 1000) {
                            let entry_offset = (pattern_id % 32) * 16;
                            let entry_bytes = entry.to_bytes();
                            block[entry_offset..entry_offset + 16]
                                .copy_from_slice(&entry_bytes);
                            let _ = sd.write_block(dir_sector, &block);
                        }

                        uart_println!("[SD] Pattern {} finalized: {} bytes at sector {}",
                            pattern_id, upload.total_size, relative_start);
                    }
                }
            }
        }

        // Handle slot status request (returns pattern directory info)
        if handler.pending_slot_status {
            handler.pending_slot_status = false;
            let mut buf = [0u8; 256]; // pattern count + summary
            let count = pattern_dir.count;
            buf[0..2].copy_from_slice(&count.to_be_bytes());
            // Pack first 16 entries (14 bytes each fits in 256)
            let mut pos = 2;
            for i in 0..(count as usize).min(16) {
                let e = &pattern_dir.entries[i];
                if pos + 14 > buf.len() { break; }
                buf[pos] = i as u8;
                buf[pos + 1] = if e.is_valid() { 0x03 } else { 0 }; // flags: valid + loaded
                buf[pos + 2..pos + 6].copy_from_slice(&e.num_vectors.to_be_bytes());
                buf[pos + 6..pos + 10].copy_from_slice(&e.size_bytes.to_be_bytes());
                buf[pos + 10..pos + 14].copy_from_slice(&e.vec_clock_hz.to_be_bytes());
                pos += 14;
            }
            let response = handler.build_slot_status_response(&buf[..pos]);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle slot invalidation (clear SD pattern directory)
        if let Some(slot_id) = handler.pending_slot_invalidate.take() {
            let _ = slot_id; // TODO: clear specific pattern or all from SD
            if slot_id == 0xFF {
                uart_println!("[SLOT] All slots invalidated");
            } else {
                uart_println!("[SLOT] Slot {} invalidated", slot_id);
            }
        }

        // =====================================================================
        // TEST PLAN HANDLERS — autonomous burn-in execution
        // =====================================================================

        // Handle SET_PLAN
        if let Some(plan) = handler.pending_set_plan.take() {
            uart_println!("[PLAN] Loaded: {} steps, loop_start={}, duration={}s",
                plan.num_steps, plan.loop_start, plan.total_duration_secs);
            plan_executor.set_plan(plan);
        }

        // Handle RUN_PLAN
        if handler.pending_run_plan {
            handler.pending_run_plan = false;
            let now_ms = fbc_firmware::hal::get_millis() as u32;
            if let Some(first_slot) = plan_executor.start(now_ms) {
                uart_println!("[PLAN] Starting — loading slot {}", first_slot);

                // Apply per-step config: temperature + clock
                let step = plan_executor.current_step();
                if step.temp_setpoint_dc != fbc_firmware::TEMP_NO_CHANGE {
                    thermal.set_target((step.temp_setpoint_dc as i32) * 100); // deci-C → milli-C
                    uart_println!("[PLAN] Step temp: {}.{}C",
                        step.temp_setpoint_dc / 10, (step.temp_setpoint_dc % 10).unsigned_abs());
                }
                if step.clock_div != fbc_firmware::CLOCK_NO_CHANGE && step.clock_div <= 4 {
                    let freq = VecClockFreq::from_hz(match step.clock_div {
                        0 => 5_000_000, 1 => 10_000_000, 2 => 25_000_000,
                        3 => 50_000_000, _ => 100_000_000,
                    });
                    clk_ctrl.set_vec_clock(freq);
                    uart_println!("[PLAN] Step clock: div={}", step.clock_div);
                }

                // Load pattern from SD → DDR region A, then parse + DMA to FPGA
                if let Some(entry) = pattern_dir.get(first_slot as u16) {
                    uart_println!("[PLAN] Loading pattern {} from SD ({} bytes)...",
                        first_slot, entry.size_bytes);
                    match ddr_buf.load_initial_from_sd(&sd, entry, first_slot as u16) {
                        Ok(size) => {
                            let (ddr_addr, fbc_size) = ddr_buf.active_region();
                            let fbc_slice = unsafe {
                                core::slice::from_raw_parts(ddr_addr as *const u8, fbc_size as usize)
                            };
                            match plan_loader.load(fbc_slice) {
                                Ok(header) => {
                                    plan_loader.start();
                                    plan_executor.on_running();
                                    handler.set_state(ControllerState::Running);
                                    plan_executor.checkpoint_to_ddr(now_ms, bim_serial_for_slots);
                                    last_checkpoint_ms = now_ms;
                                    uart_println!("[PLAN] Step 0: {} vectors @ {}Hz ({} bytes from SD)",
                                        header.num_vectors, header.vec_clock_hz, size);
                                }
                                Err(e) => {
                                    uart_println!("[PLAN] Parse .fbc failed: {:?}", e);
                                    plan_executor.stop(now_ms);
                                }
                            }
                        }
                        Err(e) => {
                            uart_println!("[PLAN] SD load failed: {:?}", e);
                            plan_executor.stop(now_ms);
                        }
                    }
                } else {
                    uart_println!("[PLAN] Pattern {} not found on SD!", first_slot);
                    plan_executor.stop(now_ms);
                }
            }
        }

        // Handle PLAN_STATUS request
        if handler.pending_plan_status {
            handler.pending_plan_status = false;
            let now_ms = fbc_firmware::hal::get_millis() as u32;
            let mut buf = [0u8; 128]; // 14 header + 8 steps × 8 bytes
            let len = plan_executor.serialize_status(&mut buf, now_ms);
            let response = handler.build_plan_status_response(&buf[..len]);
            eth.send_fbc(last_sender_mac, &response);
        }

        // Handle IO_BANK_SET — set IO bank voltage via I2C regulator
        if let Some(io_bank) = handler.pending_io_bank.take() {
            // TODO: I2C address for IO bank voltage regulator needs schematic verification
            // Sonoma's linux_IO_PS.elf uses I2C to set Bank 13/33/34/35 voltages
            // For now: log the request, ACK with "not implemented" status
            uart_println!("[IO_BANK] Set bank {} to {}mV (I2C addr TBD — needs schematic)",
                io_bank.bank, io_bank.voltage_mv);
            let ack = FbcPacket::with_payload(
                fbc_firmware::fbc_protocol::power::IO_BANK_SET_ACK,
                handler.next_seq(), &[0xFF], // 0xFF = not yet implemented
            );
            eth.send_fbc(last_sender_mac, &ack);
        }

        // Handle MIN_MAX request — XADC hardware min/max registers
        if handler.pending_min_max {
            handler.pending_min_max = false;
            let mut buf = [0u8; 32];
            if let Ok(min_max) = xadc.read_min_max() {
                for (i, (min_val, max_val)) in min_max.iter().enumerate() {
                    let off = i * 8;
                    buf[off..off+4].copy_from_slice(&min_val.to_be_bytes());
                    buf[off+4..off+8].copy_from_slice(&max_val.to_be_bytes());
                }
            }
            let response = FbcPacket::with_payload(
                fbc_firmware::fbc_protocol::runtime::MIN_MAX_RSP,
                handler.next_seq(), &buf,
            );
            eth.send_fbc(last_sender_mac, &response);
        }

        // Plan execution: check if vectors finished and advance to next step
        if plan_executor.state == fbc_firmware::PlanState::Running {
            if fbc.is_done() {
                let now_ms = fbc_firmware::hal::get_millis() as u32;
                let errors = status.get_error_count();
                let action = plan_executor.on_vectors_done(errors, now_ms);

                match action {
                    fbc_firmware::PlanAction::LoadPattern(pattern_id) => {
                        // Apply per-step config before loading vectors
                        let step = plan_executor.current_step();
                        if step.temp_setpoint_dc != fbc_firmware::TEMP_NO_CHANGE {
                            thermal.set_target((step.temp_setpoint_dc as i32) * 100);
                        }
                        if step.clock_div != fbc_firmware::CLOCK_NO_CHANGE && step.clock_div <= 4 {
                            let freq = VecClockFreq::from_hz(match step.clock_div {
                                0 => 5_000_000, 1 => 10_000_000, 2 => 25_000_000,
                                3 => 50_000_000, _ => 100_000_000,
                            });
                            clk_ctrl.set_vec_clock(freq);
                        }

                        // Begin chunked SD → DDR load (non-blocking, pumped by main loop)
                        if let Some(entry) = pattern_dir.get(pattern_id as u16) {
                            match ddr_buf.begin_load(entry, pattern_id as u16) {
                                Ok(state) => {
                                    pending_sd_load = Some(state);
                                    uart_println!("[PLAN] Loading pattern {} from SD ({} sectors)...",
                                        pattern_id, entry.sector_count());
                                }
                                Err(e) => {
                                    uart_println!("[PLAN] begin_load pattern {} failed: {:?}",
                                        pattern_id, e);
                                    plan_executor.stop(now_ms);
                                    handler.set_state(ControllerState::Error);
                                }
                            }
                        } else {
                            uart_println!("[PLAN] Pattern {} not on SD!", pattern_id);
                            plan_executor.stop(now_ms);
                            handler.set_state(ControllerState::Error);
                        }
                    }
                    fbc_firmware::PlanAction::PlanComplete => {
                        uart_println!("[PLAN] Complete — {} loops", plan_executor.plan_loops);
                        handler.set_state(ControllerState::Done);
                        plan_executor.clear_checkpoint();
                        let result = &plan_executor.results[plan_executor.current_step as usize];
                        let pkt = handler.build_step_result(result);
                        eth.send_fbc(last_sender_mac, &pkt);
                    }
                    fbc_firmware::PlanAction::PlanAborted => {
                        uart_println!("[PLAN] Aborted at step {}", plan_executor.current_step);
                        handler.set_state(ControllerState::Error);
                        plan_executor.clear_checkpoint();
                        let result = &plan_executor.results[plan_executor.current_step as usize];
                        let pkt = handler.build_step_result(result);
                        eth.send_fbc(last_sender_mac, &pkt);
                    }
                    fbc_firmware::PlanAction::None => {}
                }

                // Checkpoint to DDR every ~10s during active plan
                if now_ms.wrapping_sub(last_checkpoint_ms) >= 10_000 {
                    plan_executor.checkpoint_to_ddr(now_ms, bim_serial_for_slots);
                    last_checkpoint_ms = now_ms;
                }
            }
        }

        // Handle pending CONFIGURE (clock frequency)
        // clk_ctrl AXI crash FIXED March 25 — root cause was incomplete `case` in
        // AXI write FSM (missing `default:` arms). Verified on hardware.
        if let Some(config) = handler.take_pending_config() {
            if clk_ctrl.is_accessible() {
                let freq = VecClockFreq::from_hz(match config.clock_div {
                    0 => 5_000_000,
                    1 => 10_000_000,
                    2 => 25_000_000,
                    3 => 50_000_000,
                    4 => 100_000_000,
                    _ => 50_000_000,
                });
                clk_ctrl.set_vec_clock(freq);
                uart_println!("[CFG] Vector clock set to {} MHz", freq.to_hz() / 1_000_000);
            } else {
                uart_println!("[CFG] Clock control NOT accessible — skipping (needs bitstream fix)");
            }
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
                handler.log_index = recorder.sequence();
                let heartbeat = handler.build_heartbeat();
                eth.send_fbc(last_sender_mac, &heartbeat);

                // Flight Recorder: Log heartbeat via corruption-resistant recorder
                if recorder.sd_ok {
                    let hb_bytes: &[u8] = unsafe {
                        core::slice::from_raw_parts(
                            &heartbeat as *const _ as *const u8,
                            core::mem::size_of_val(&heartbeat)
                        )
                    };
                    let _ = recorder.write_heartbeat(&sd, hb_bytes);
                }
            }
        } else {
            heartbeat_counter = 0;

            // Idle heartbeat: broadcast ANNOUNCE so `fbc-cli monitor` can see us
            idle_heartbeat_counter += 1;
            if idle_heartbeat_counter >= IDLE_HEARTBEAT_INTERVAL {
                idle_heartbeat_counter = 0;

                // Network health: check PHY link status, re-announce if up
                if eth.link_up() {
                    let announce = handler.build_announce();
                    eth.send_fbc(BROADCAST_MAC, &announce);
                } else {
                    uart_println!("[NET] Link down — waiting for PHY recovery");
                }
            }
        }

        // =====================================================================
        // SD → DDR CHUNK PUMP — non-blocking pattern load
        // Reads 32KB per iteration (~1.3ms). Safety monitor runs between chunks.
        // =====================================================================
        if let Some(ref mut state) = pending_sd_load {
            match ddr_buf.load_chunk(&sd, state) {
                Ok(fbc_firmware::LoadProgress::Done) => {
                    // All sectors loaded — swap regions and start vectors
                    let (ddr_addr, fbc_size) = ddr_buf.swap();
                    let fbc_slice = unsafe {
                        core::slice::from_raw_parts(ddr_addr as *const u8, fbc_size as usize)
                    };
                    match plan_loader.load(fbc_slice) {
                        Ok(_) => {
                            plan_loader.start();
                            plan_executor.on_running();
                            uart_println!("[PLAN] Pattern loaded, vectors running");
                        }
                        Err(e) => {
                            uart_println!("[PLAN] Parse .fbc after SD load failed: {:?}", e);
                            let now_ms = fbc_firmware::hal::get_millis() as u32;
                            plan_executor.stop(now_ms);
                            handler.set_state(ControllerState::Error);
                        }
                    }
                    pending_sd_load = None;
                }
                Ok(fbc_firmware::LoadProgress::InProgress { .. }) => {
                    // More chunks needed — will continue next iteration
                }
                Err(e) => {
                    uart_println!("[PLAN] SD chunk read failed: {:?}", e);
                    let now_ms = fbc_firmware::hal::get_millis() as u32;
                    plan_executor.stop(now_ms);
                    handler.set_state(ControllerState::Error);
                    pending_sd_load = None;
                }
            }
        }

        // =====================================================================
        // SAFETY MONITOR — runs regardless of GUI connection
        // Checks die temperature + VICOR current against limits.
        // On violation: emergency stop ALL power, latch error, broadcast alert.
        // This is the controller protecting itself and the DUT autonomously.
        // =====================================================================
        safety_counter += 1;
        if safety_counter >= SAFETY_CHECK_INTERVAL && !safety_tripped {
            safety_counter = 0;

            // Check 1: Temperature — prefer THERM_CASE (NTC on BIM) over XADC die temp
            // AnalogMonitor auto-disables external ADC on first SPI failure (blown BIM etc.)
            let temp_mc = analog_monitor.read_case_temp_mc()
                .unwrap_or_else(|_| xadc.read_temperature_millicelsius().unwrap_or(25_000));
            {
                // Update thermal controller with real-time V×I power feedback
                if let Ok((_total_mw, power_level)) = analog_monitor.read_core_power_mw() {
                    thermal.set_power_level(power_level);
                }

                // Feed thermal controller with case temperature
                let thermal_out = thermal.update(temp_mc);
                let heater_duty = output_to_heater(thermal_out.correction);
                let fan_duty = output_to_fan(thermal_out.correction);
                // Thermal control via BU2505 DAC: ch1=HEATER, ch0=COOLER
                // Duty 0-100% → DAC 0-4096mV (comparator on BIM switches FET)
                let heater_mv = (heater_duty as u16) * 41; // 100% → 4100mV ≈ full scale
                let fan_mv = (fan_duty as u16) * 41;
                let _ = dac.set_voltage_mv(1, heater_mv); // HEATER_DAC_CHANNEL = 1
                let _ = dac.set_voltage_mv(0, fan_mv);    // COOLER_DAC_CHANNEL = 0

                // Convert to 0.1°C for comparison with board_config
                let temp_dc = (temp_mc / 100) as i16;
                let shutdown_dc = board_config.temp_shutdown_dc();
                if temp_dc > shutdown_dc {
                    uart_println!("[SAFETY] OVER-TEMPERATURE: {}°C > {}°C — EMERGENCY STOP",
                        temp_dc / 10, shutdown_dc / 10);
                    vicor.disable_all();
                    psu_mgr.disable_all();
                    safety_tripped = true;

                    // Broadcast error so any listener sees it
                    let error_pkt = handler.build_error(
                        1,  // error_type: 1 = over-temperature
                        0,
                        temp_dc as u32,
                    );
                    eth.send_fbc(BROADCAST_MAC, &error_pkt);
                }
            }

            // Check 2: XADC over-temperature hardware flag (catches sensor failures too)
            if xadc.is_over_temperature() && !safety_tripped {
                uart_println!("[SAFETY] XADC OT flag asserted — EMERGENCY STOP");
                vicor.disable_all();
                psu_mgr.disable_all();
                safety_tripped = true;

                let error_pkt = handler.build_error(1, 0, 0);
                eth.send_fbc(BROADCAST_MAC, &error_pkt);
            }

            // Check 3: VICOR core voltage out of range (catches regulator faults)
            // Only check if we have valid EEPROM limits
            if board_config.has_eeprom() && !safety_tripped {
                let vicor_status = vicor.get_status();
                for (i, (enabled, voltage_mv)) in vicor_status.iter().enumerate() {
                    if !enabled { continue; }
                    let rail = board_config.effective_rail(i);
                    if rail.max_voltage_mv == 0 { continue; } // No limit configured

                    if *voltage_mv > rail.max_voltage_mv + 100 {
                        // +100mV margin for transient spikes
                        uart_println!("[SAFETY] VICOR core {} overvoltage: {}mV > {}mV — EMERGENCY STOP",
                            i + 1, voltage_mv, rail.max_voltage_mv);
                        vicor.disable_all();
                        psu_mgr.disable_all();
                        safety_tripped = true;

                        let error_pkt = handler.build_error(
                            2,  // error_type: 2 = over-voltage
                            (i + 1) as u32,
                            *voltage_mv as u32,
                        );
                        eth.send_fbc(BROADCAST_MAC, &error_pkt);
                        break;
                    }

                    // Undervoltage: supply sagging below minimum (dying regulator, short, etc.)
                    // Only check if min_voltage configured and supply is reading nonzero
                    // (zero = ADC not reading, not a sag)
                    if rail.min_voltage_mv > 0 && *voltage_mv > 0
                        && *voltage_mv < rail.min_voltage_mv.saturating_sub(100)
                    {
                        uart_println!("[SAFETY] VICOR core {} undervoltage: {}mV < {}mV — EMERGENCY STOP",
                            i + 1, voltage_mv, rail.min_voltage_mv);
                        vicor.disable_all();
                        psu_mgr.disable_all();
                        safety_tripped = true;

                        let error_pkt = handler.build_error(
                            3,  // error_type: 3 = under-voltage
                            (i + 1) as u32,
                            *voltage_mv as u32,
                        );
                        eth.send_fbc(BROADCAST_MAC, &error_pkt);
                        break;
                    }
                }
            }
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
/// Halt CPU on fatal error. No LEDs on this board — just loops forever.
/// Pattern parameter kept for future boards that may have LEDs.
fn hang_with_blink(_pattern: u32) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// =============================================================================
// Startup Code (ARM Assembly)
// =============================================================================

// Include ARM assembly startup code
// This handles: stack setup, BSS clear, FPU enable, then calls main()
core::arch::global_asm!(include_str!("boot.S"));
