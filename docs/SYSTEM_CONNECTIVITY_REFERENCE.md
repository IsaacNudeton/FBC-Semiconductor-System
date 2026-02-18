# Sonoma Burn-In System - Complete Connectivity Reference

> **Purpose:** Document how controllers connect to BIMs, Quad Boards, and the system infrastructure
> **Last Updated:** 2026-02-09 (Corrected based on actual hardware BOMs)

---

## TABLE OF CONTENTS

1. [System Overview](#1-system-overview)
2. [Physical Hierarchy](#2-physical-hierarchy)
3. [Network Infrastructure](#3-network-infrastructure)
4. [Controller to BIM Connection](#4-controller-to-bim-connection)
5. [Quad Board Architecture](#5-quad-board-architecture)
6. [BIM Interface Specification](#6-bim-interface-specification)
7. [Calibration Board Deep Dive](#7-calibration-board-deep-dive)
8. [Power Distribution](#8-power-distribution)
9. [Signal Routing](#9-signal-routing)
10. [Repurposing Guide](#10-repurposing-guide)

---

## 1. SYSTEM OVERVIEW

```
COMPLETE SONOMA BURN-IN SYSTEM ARCHITECTURE
============================================

+===========================================================================+
|                              HOST PC                                       |
|                           (172.16.0.49)                                    |
|                                                                            |
|  +-------------------+  +-------------------+  +---------------------+     |
|  |   Everest         |  |   Unity Setup     |  |   Pattern Tools     |     |
|  |   (Test Control)  |  |   Editor          |  |   (STIL/AVC->HEX)   |     |
|  +-------------------+  +-------------------+  +---------------------+     |
+===========================================================================+
                                    |
                                    | Gigabit Ethernet
                                    v
+===========================================================================+
|                        CISCO 4948 ETHERNET SWITCH                          |
|                           (DHCP Server)                                    |
|  96 Ports Available                                                        |
|  Front Tray: 172.16.0.101 - 172.16.0.148                                  |
|  Back Tray:  172.16.0.201 - 172.16.0.248                                  |
+===========================================================================+
          |              |              |              |
          v              v              v              v
+=========================================================================+
|    RACK 1         RACK 2         RACK 3    ...    RACK 11               |
|  +----------+   +----------+   +----------+     +----------+            |
|  |Front Tray|   |Front Tray|   |Front Tray|     |Front Tray|            |
|  | 4 BIMs   |   | 4 BIMs   |   | 4 BIMs   |     | 4 BIMs   |            |
|  +----------+   +----------+   +----------+     +----------+            |
|  |Back Tray |   |Back Tray |   |Back Tray |     |Back Tray |            |
|  | 4 BIMs   |   | 4 BIMs   |   | 4 BIMs   |     | 4 BIMs   |            |
|  +----------+   +----------+   +----------+     +----------+            |
+=========================================================================+
                                    |
                                    v
              +=============================================+
              |              PER-BIM STACK                  |
              +=============================================+
              |                                             |
              |  +---------------------------------------+  |
              |  |         CONTROLLER BOARD              |  |
              |  |         (HPBI Controller RevD)        |  |
              |  |  - Zynq 7020 SoC (ARM + FPGA)        |  |
              |  |  - Ethernet (RJ45)                   |  |
              |  |  - QSH connectors to BIM (signals)   |  |
              |  |  - Power input from Quad Board       |  |
              |  +---------------------------------------+  |
              |          |                    |             |
              |          | QSH               | Power        |
              |          | (GPIO Signals)    | Rails        |
              |          v                    v             |
              |  +----------------+  +----------------+     |
              |  |     BIM        |  |  QUAD BOARD    |     |
              |  | (Signals+DUT)  |  | (Power Only)   |     |
              |  +----------------+  +----------------+     |
              |          |                    |             |
              |          v                    v             |
              |  +---------------------------------------+  |
              |  |              DUT SOCKET               |  |
              |  |  (Device Under Test on BIM)          |  |
              |  +---------------------------------------+  |
              +=============================================+
```

**IMPORTANT CORRECTION (2026-02-09):**
- QSH connectors carry **signals directly from Controller to BIM**
- Quad Board is for **power distribution only** (VICOR, LCPS rails)
- Signals do NOT route through Quad Board

---

## 2. PHYSICAL HIERARCHY

```
PHYSICAL HIERARCHY - TOP TO BOTTOM
==================================

SYSTEM LEVEL:
+------------------------------------------------------------------+
|  SONOMA BURN-IN SYSTEM                                            |
|  - 1x Host PC (Everest Software)                                 |
|  - 1x Cisco 4948 Switch (96 ports)                               |
|  - Up to 11 Racks                                                |
|  - Maximum 88 BIMs (11 racks x 2 trays x 4 BIMs)                |
+------------------------------------------------------------------+
            |
            v
RACK LEVEL:
+------------------------------------------------------------------+
|  SINGLE RACK                                                      |
|  - 2 Trays (Front + Back)                                        |
|  - Each tray holds 1 Quad Board + up to 4 BIMs                   |
|  - Controller per BIM (or shared for multi-DUT BIM)              |
|  - Power distribution unit                                        |
|  - Cooling system (water/air)                                    |
+------------------------------------------------------------------+
            |
            v
TRAY LEVEL:
+------------------------------------------------------------------+
|  SINGLE TRAY                                                      |
|  +------------------------------------------------------------+  |
|  |  CONTROLLER(S)        |  QUAD BOARD (Power)                |  |
|  |  +------+ +------+    |  +----------------------------+    |  |
|  |  |Ctrl 1| |Ctrl 2|    |  | VICOR DC-DC (48V→12V)     |    |  |
|  |  +------+ +------+    |  | Capacitors, LCPS rails     |    |  |
|  |  +------+ +------+    |  +----------------------------+    |  |
|  |  |Ctrl 3| |Ctrl 4|    |                                    |  |
|  |  +------+ +------+    |                                    |  |
|  +------------------------------------------------------------+  |
|  |  BIM SLOTS (signals from controllers, power from Quad)     |  |
|  |  +----------+  +----------+  +----------+  +----------+    |  |
|  |  | BIM Slot | | BIM Slot | | BIM Slot | | BIM Slot |     |  |
|  |  |    1     | |    2     | |    3     | |    4     |     |  |
|  |  +----------+  +----------+  +----------+  +----------+    |  |
|  |       OR: Single Quad-Sized BIM (spans all 4 slots)        |  |
|  +------------------------------------------------------------+  |
+------------------------------------------------------------------+
            |
            v
BIM LEVEL:
+------------------------------------------------------------------+
|  BIM SIZES:                                                       |
|                                                                   |
|  SINGLE BIM:        HALF BIM:          QUAD BIM:                 |
|  +--------+         +--------+         +------------------+       |
|  |125x125 |         |125x250 |         |      250x250     |       |
|  |  mm    |         |  mm    |         |        mm        |       |
|  | 1 DUT  |         | 2 DUT  |         |   1-6 DUT or     |       |
|  +--------+         +--------+         |   Calibration    |       |
|  Uses 1 slot        Uses 2 slots       +------------------+       |
|                                        Uses all 4 slots           |
+------------------------------------------------------------------+
```

---

## 3. NETWORK INFRASTRUCTURE

```
NETWORK TOPOLOGY DETAIL
=======================

+-------------------------------------------------------------------------+
|                          HOST PC (172.16.0.49)                           |
|  +-------------------------------------------------------------------+  |
|  |  Dual Network Interface:                                          |  |
|  |  - NIC 1: 172.16.0.49 (Sonoma internal network)                  |  |
|  |  - NIC 2: Enterprise LAN / WiFi (external access)                |  |
|  +-------------------------------------------------------------------+  |
+-------------------------------------------------------------------------+
                                    |
                                    | Gigabit Ethernet
                                    | Cat6 Cable
                                    v
+-------------------------------------------------------------------------+
|                     CISCO CATALYST 4948 SWITCH                           |
|                                                                          |
|  Configuration:                                                          |
|  - 48 x 10/100/1000 ports (2 switches = 96 ports)                       |
|  - Built-in DHCP server                                                 |
|  - VLAN: 172.16.0.0/16 (internal only, not routed)                     |
|                                                                          |
|  Port Mapping:                                                           |
|  +-------+------------------+------------------+                        |
|  | Port  | Front Tray IP    | Back Tray IP     |                        |
|  +-------+------------------+------------------+                        |
|  |   1   | 172.16.0.101     | 172.16.0.201     |                        |
|  |   2   | 172.16.0.102     | 172.16.0.202     |                        |
|  |   3   | 172.16.0.103     | 172.16.0.203     |                        |
|  |  ...  | ...              | ...              |                        |
|  |  48   | 172.16.0.148     | 172.16.0.248     |                        |
|  +-------+------------------+------------------+                        |
|                                                                          |
|  Note: IP address determined by physical port connection                 |
|        (MAC address registered on first boot)                           |
+-------------------------------------------------------------------------+
          |         |         |         |         |
          v         v         v         v         v
    +----------+ +----------+ +----------+ +----------+
    |Controller| |Controller| |Controller| |Controller|  ...
    |172.16.0. | |172.16.0. | |172.16.0. | |172.16.0. |
    |   101    | |   102    | |   103    | |   104    |
    +----------+ +----------+ +----------+ +----------+


SSH ACCESS:
===========
$ ssh root@172.16.0.xxx
Password: root

zynq> cd /mnt
zynq> ls linux*.elf
linux_XADC.elf  linux_pin_type.elf  linux_EXT_DAC.elf ...

zynq> /mnt/linux_dut_present.elf
0    # (0 = DUT present, -1 = not present)
```

---

## 4. CONTROLLER TO BIM CONNECTION

**CRITICAL: Signals go DIRECTLY from Controller to BIM via QSH connectors.**
**The Quad Board handles POWER ONLY.**

```
CONTROLLER <-> BIM SIGNAL INTERFACE (DIRECT CONNECTION)
=======================================================

CONTROLLER BOARD (HPBI Controller RevD / Sonoma Controller)
+------------------------------------------------------------------+
|                                                                   |
|  +-------------+  +------------------+  +--------------------+   |
|  | Zynq 7020   |  | Power Input      |  | Ethernet PHY       |   |
|  | SoC         |  | (from Quad Bd)   |  | (RJ45)             |   |
|  +-------------+  +------------------+  +--------------------+   |
|         |                                                         |
|         | GPIO Banks 13/33/34/35                                 |
|         v                                                         |
|  +----------------------------------------------------------+    |
|  |              QSH CONNECTOR ARRAY (to BIM)                 |    |
|  |  +------+  +------+  +------+  +------+                  |    |
|  |  | J3   |  | J4   |  | J5   |  | J6   |                  |    |
|  |  |Signals| |Signals| |Signals| |ADC/  |                  |    |
|  |  |0-47  |  |48-95 |  |96-127|  |DAC   |                  |    |
|  |  +------+  +------+  +------+  +------+                  |    |
|  +----------------------------------------------------------+    |
|                                                                   |
+------------------------------------------------------------------+
                              ||
                              || QSH-090-XX-X-D-A Connectors
                              || (High-density 0.5mm pitch)
                              || **DIRECTLY TO BIM**
                              vv
BIM (BURN-IN MODULE)
+------------------------------------------------------------------+
|  +----------------------------------------------------------+    |
|  |              QTH RECEPTACLE ARRAY (from Controller)       |    |
|  |  +------+  +------+  +------+  +------+                  |    |
|  |  | J3   |  | J4   |  | J5   |  | J6   |                  |    |
|  |  +------+  +------+  +------+  +------+                  |    |
|  +----------------------------------------------------------+    |
|                                                                   |
|         +------------------------------------------+             |
|         |           DUT SOCKET AREA                |             |
|         |           (125mm x 125mm)                |             |
|         |                                          |             |
|         |      +------------------+                |             |
|         |      |       DUT        |                |             |
|         |      +------------------+                |             |
|         +------------------------------------------+             |
|                                                                   |
+------------------------------------------------------------------+


QSH CONNECTOR SPECIFICATIONS (Samtec):
======================================

Part Number: QSH-090-01-L-D-A (typical)
  - 0.50mm pitch
  - 90 positions per row (180 total for -D dual row)
  - Current: 2A per signal pin, 25A per ground plane
  - Voltage: 175 VAC
  - Data rate: Up to 25 Gbps
  - Mating cycles: 100

Connector Allocation:
+--------+--------------------+---------------------------+
| Conn   | Signals            | Purpose                   |
+--------+--------------------+---------------------------+
| J3     | GPIO[0:47]         | Bank 13 signals to DUT    |
| J4     | GPIO[48:95]        | Bank 33 signals to DUT    |
| J5     | GPIO[96:127]       | Bank 34 signals to DUT    |
| J6     | ADC/DAC/I2C/SPI    | Analog + control signals  |
+--------+--------------------+---------------------------+
```

---

## 5. QUAD BOARD ARCHITECTURE

**The Quad Board is for POWER DISTRIBUTION ONLY.**
**It does NOT route GPIO signals.**

```
QUAD BOARD - POWER DISTRIBUTION
===============================

Part Number: 02291B_QUAD_BOARD (Rev B, dated 01/07/2020)
Function: 48V to 12V DC-DC conversion and power distribution

QUAD BOARD BLOCK DIAGRAM:
+------------------------------------------------------------------+
|                                                                   |
|  48V INPUT (from rack PDU)                                       |
|       |                                                          |
|       v                                                          |
|  +----------------------------------------------------------+   |
|  |  VICOR BCM48BF120T300A00 DC-DC CONVERTERS (x4)           |   |
|  |  - Input: 48V                                             |   |
|  |  - Output: 12V @ 300W each                                |   |
|  |  - Efficiency: 96%                                        |   |
|  |  - Locations: VR1, VR2, VR3, VR4                         |   |
|  +----------------------------------------------------------+   |
|       |                                                          |
|       v                                                          |
|  +----------------------------------------------------------+   |
|  |  BULK CAPACITORS                                          |   |
|  |  - 470µF 63V Electrolytic (C5-C12) x8                    |   |
|  |  - 470µF 16V Tantalum (C13-C20) x8                       |   |
|  |  - 47µF 25V Ceramic (C1-C4) x4                           |   |
|  +----------------------------------------------------------+   |
|       |                                                          |
|       v                                                          |
|  +----------------------------------------------------------+   |
|  |  POWER DISTRIBUTION TO BIMs                               |   |
|  |  - 12V rails to each BIM slot                            |   |
|  |  - Ground planes                                          |   |
|  |  - Current sense (CTEST1-4)                              |   |
|  +----------------------------------------------------------+   |
|       |                                                          |
|       v                                                          |
|  +----------+  +----------+  +----------+  +----------+         |
|  | BIM Slot | | BIM Slot | | BIM Slot | | BIM Slot |          |
|  |  1 PWR   | |  2 PWR   | |  3 PWR   | |  4 PWR   |          |
|  +----------+  +----------+  +----------+  +----------+         |
|                                                                   |
+------------------------------------------------------------------+


QUAD BOARD BOM SUMMARY (02291B, Rev B):
=======================================

| Qty | Part              | Description                    | Location     |
|-----|-------------------|--------------------------------|--------------|
| 1   | 02291 PCB         | Sonoma Quad Board bare PCB     | -            |
| 4   | BCM48BF120T300A00 | VICOR 48V→12V 300W DC-DC      | VR1-VR4      |
| 8   | 32441 Heatsink    | VICOR BCM heatsink             | VR1-VR4      |
| 8   | 32435 Push Pin    | Heatsink mounting              | VR1-VR4      |
| 8   | 470µF 63V         | Electrolytic capacitor         | C5-C12       |
| 8   | 470µF 16V         | Tantalum capacitor             | C13-C20      |
| 4   | 47µF 25V          | Ceramic capacitor              | C1-C4        |
| 1   | Toggle Switch     | SPDT 0.4VA 20V                 | SW1          |
| 3   | LED               | Status indicators              | LED1-3       |
| 6   | Molex 22-05-3031  | Test/I2C connectors            | CTEST1-4,I2C |

**NOTE: No QSH connectors, no GPIO routing components.**
```

---

## 6. BIM INTERFACE SPECIFICATION

```
BIM (BURN-IN MODULE) INTERFACE
==============================

CONNECTIONS TO BIM:
===================

1. SIGNAL CONNECTION (from Controller via QSH):
   - 128 GPIO pins (directly from Controller FPGA)
   - ADC inputs (for current/voltage sense)
   - DAC outputs (for programmable levels)
   - I2C, SPI, JTAG control signals

2. POWER CONNECTION (from Quad Board):
   - 12V main rail
   - Ground planes
   - (BIM has local regulators for DUT-specific voltages)


STANDARD BIM PINOUT:
====================

From Controller (QSH connectors):
+------------------------------------------------------------------+
| Signal Group      | Pin Range        | Count | Purpose            |
+-------------------+------------------+-------+--------------------+
| GPIO Bank 13      | GPIO[0:47]       | 48    | DUT I/O            |
| GPIO Bank 33      | GPIO[48:95]      | 48    | DUT I/O            |
| GPIO Bank 34      | GPIO[96:127]     | 32    | DUT I/O            |
| ADC Inputs        | ADC[0:15]        | 16    | Voltage/current    |
| DAC Outputs       | DAC[0:9]         | 10    | Programmable refs  |
| I2C               | SCL, SDA         | 2     | EEPROM, sensors    |
| SPI               | MOSI,MISO,SCK,CS | 4     | External ADC/DAC   |
| JTAG              | TDI,TDO,TCK,TMS  | 4     | DUT JTAG           |
+------------------------------------------------------------------+

From Quad Board (Power):
+------------------------------------------------------------------+
| Rail              | Voltage          | Current | Purpose          |
+-------------------+------------------+---------+------------------+
| VCC12             | 12V              | High    | Main power       |
| GND               | 0V               | -       | Ground reference |
+------------------------------------------------------------------+


BIM EEPROM (Per-BIM Identification):
====================================
Each BIM has an I2C EEPROM storing:
- BIM type ID
- Serial number
- Hardware revision
- Manufacturing date
- Calibration data

Access: I2C address 0x50 (24LC02)
```

---

## 7. CALIBRATION BOARD DEEP DIVE

```
CALIBRATION BOARD (QUAD-SIZED BIM)
==================================

Part Number: 02292C02_CALIB_BOARD
Design File: CalibrationBoard05Dec2018

PURPOSE:
--------
The Calibration Board is a Quad-sized BIM used for:
- System validation and calibration
- Controller board verification
- ADC/DAC characterization
- GPIO testing
- Power supply validation

PHYSICAL SPECIFICATIONS:
========================
Size:        ~250mm x 250mm (occupies all 4 BIM slots)
Layers:      8-layer PCB

CONNECTIONS:
============
- Signal: QSH connectors directly from Controller
- Power: 12V rails from Quad Board


CALIBRATION BOARD I/O CAPABILITIES:
===================================

1. GPIO TESTING (128 pins):
   +------------------+------------------+--------------------+
   | Signal Group     | Pin Range        | Test Capability    |
   +------------------+------------------+--------------------+
   | GPIO Bank 13     | GPIO[0:47]       | Loopback testing   |
   | GPIO Bank 33     | GPIO[48:95]      | Level verification |
   | GPIO Bank 34     | GPIO[96:127]     | Frequency test     |
   +------------------+------------------+--------------------+

2. ANALOG I/O:
   +------------------+------------------+--------------------+
   | Interface        | Channels         | Resolution         |
   +------------------+------------------+--------------------+
   | XADC (internal)  | 16               | 12-bit, ~1 MSPS   |
   | External ADC     | 16 (MAX11131)    | 12-bit, 3 MSPS    |
   | External DAC     | 10 (BU2505)      | 10-bit            |
   +------------------+------------------+--------------------+

3. COMMUNICATION:
   +------------------+--------------------+
   | Interface        | Purpose            |
   +------------------+--------------------+
   | UART             | Debug console      |
   | I2C Bus 0        | EEPROM, sensors    |
   | I2C Bus 1        | PMBus supplies     |
   | SPI Bus 0        | External ADC/DAC   |
   | JTAG             | FPGA debug         |
   +------------------+--------------------+
```

---

## 8. POWER DISTRIBUTION

```
POWER DISTRIBUTION ARCHITECTURE
===============================

RACK POWER INPUT
       |
       v
+------------------------------------------------------------------+
|                    RACK POWER DISTRIBUTION                        |
|                                                                   |
|  AC Input --> PDU --> 48V DC Bus                                 |
|                                                                   |
+------------------------------------------------------------------+
       |
       | 48V DC
       v
+------------------------------------------------------------------+
|                    QUAD BOARD (per tray)                          |
|                                                                   |
|  48V --> VICOR BCM (x4) --> 12V @ 1200W total                    |
|                                                                   |
|  Distributes 12V to up to 4 BIMs                                 |
+------------------------------------------------------------------+
       |
       | 12V
       v
+------------------------------------------------------------------+
|                    BIM LOCAL POWER                                |
|                                                                   |
|  12V --> Local regulators --> DUT-specific voltages              |
|                                                                   |
|  Examples:                                                        |
|  - 3.3V for I/O                                                  |
|  - 1.8V for core                                                 |
|  - 0.9V for low-power core                                       |
|                                                                   |
+------------------------------------------------------------------+
       |
       v
+------------------------------------------------------------------+
|                    DUT POWER                                      |
|                                                                   |
|  Regulated voltages to Device Under Test                         |
|                                                                   |
+------------------------------------------------------------------+


CONTROLLER POWER SUPPLIES (separate from Quad Board):
=====================================================

The Controller board has its own power supply modules:

| Supply Type | Controller | Current | Purpose           |
|-------------|------------|---------|-------------------|
| LC (LCPS)   | PMBus I2C  | 12A ea  | General supplies  |
| HC (HCPS)   | PMBus I2C  | 40A ea  | High-current      |
| VICOR Core  | MIO + DAC  | 50A+    | Processor core    |

Control Commands (Linux firmware):
  /mnt/linux_pmbus_PicoDlynx.elf <ch> <voltage>
  /mnt/linux_VICOR.elf <voltage>
  /mnt/linux_EXT_DAC.elf <v0> <v1> ... <v9>
```

---

## 9. SIGNAL ROUTING

```
SIGNAL ROUTING: CONTROLLER -> BIM -> DUT
========================================

**KEY POINT: GPIO signals go DIRECTLY from Controller to BIM.**
**They do NOT pass through the Quad Board.**


PATH 1: GPIO SIGNALS (Direct)
=============================

Controller FPGA                          BIM
+------------------+                    +------------------+
|                  |                    |                  |
| Bank 13 GPIO     |======= QSH J3 ===>| GPIO[0:47]       |
| (GPIO[0:47])     |      (direct)     | to DUT           |
|                  |                    |                  |
| Bank 33 GPIO     |======= QSH J4 ===>| GPIO[48:95]      |
| (GPIO[48:95])    |      (direct)     | to DUT           |
|                  |                    |                  |
| Bank 34 GPIO     |======= QSH J5 ===>| GPIO[96:127]     |
| (GPIO[96:127])   |      (direct)     | to DUT           |
|                  |                    |                  |
+------------------+                    +------------------+


PATH 2: POWER (Through Quad Board)
==================================

Rack PDU        Quad Board              BIM
+-------+      +---------------+       +--------+
| 48V   |=====>| VICOR DC-DC   |======>| 12V    |
|       |      | 48V -> 12V    |       | Rails  |
+-------+      +---------------+       +--------+


GPIO PIN TYPE CONFIGURATION:
============================

Configure pin behavior using linux_pin_type.elf:

+------+------------+-----------------------------------------------+
| Type | Name       | Behavior                                      |
+------+------------+-----------------------------------------------+
|  0   | BIDIR      | Bidirectional - direction set per vector     |
|  1   | INPUT      | Controller input (reads DUT output)          |
|  2   | OUTPUT     | Controller output (drives DUT input)         |
|  3   | OPEN_C     | Open collector - only drives low             |
|  4   | PULSE      | Positive clock pulse (programmable timing)   |
|  5   | NPULSE     | Negative clock pulse (programmable timing)   |
|  6   | ERROR_TRIG | Error trigger input                          |
|  7   | VEC_CLK    | Free-running vector clock                    |
|  8   | VEC_CLK_EN | Vector clock enable                          |
+------+------------+-----------------------------------------------+

Example:
  /mnt/linux_pin_type.elf 0 2    # GPIO 0 as output
  /mnt/linux_pin_type.elf 1 1    # GPIO 1 as input


I/O VOLTAGE LEVELS:
===================

Configure VIH reference per bank:

/mnt/linux_IO_PS.elf <vih1> <vih2> <vih3> <vih4>

Where:
  vih1 = Reference for GPIO[0:47]    (Bank 13)
  vih2 = Reference for GPIO[48:95]   (Bank 33)
  vih3 = Reference for GPIO[96:127]  (Bank 34)
  vih4 = Reserved

Range: 0.8V to 3.6V
Default: LVCMOS25 (2.5V)
```

---

## 10. REPURPOSING GUIDE

```
REPURPOSING THE CONTROLLER + CALIBRATION BOARD
==============================================

The Calibration Board provides an excellent platform for repurposing
because it exposes all controller capabilities with test points.

AVAILABLE RESOURCES:
====================

1. GPIO (128 pins):
   - All configurable as input, output, or special function
   - Max toggle rate: ~100 MHz
   - I/O standard: LVCMOS25 (2.5V)

2. ANALOG I/O:
   - 16x XADC (0-1V differential, 12-bit)
   - 16x External ADC (0-3V, 12-bit, MAX11131)
   - 10x DAC (0-4.096V, 10-bit, BU2505)

3. COMMUNICATION:
   - 2x I2C buses
   - 2x SPI buses
   - 1x UART
   - JTAG

4. POWER:
   - LC/HC programmable supplies via PMBus
   - VICOR high-current core supply


ACCESSING THE CONTROLLER:
=========================

$ ssh root@172.16.0.xxx
Password: root

# Set all pins as outputs
for i in $(seq 0 127); do
  /mnt/linux_pin_type.elf $i 2
done

# Set voltage levels
/mnt/linux_IO_PS.elf 3.3 3.3 3.3 3.3

# Read ADCs
/mnt/linux_XADC.elf
/mnt/linux_EXT_ADC.elf

# Set DAC outputs
/mnt/linux_EXT_DAC.elf 1.0 2.0 3.0 4.0 1.5 2.5 3.5 4.096 0.5 0.0
```

---

## APPENDIX A: CONNECTOR PART NUMBERS

```
CONNECTOR SPECIFICATIONS
========================

QSH/QTH CONNECTORS (Controller <-> BIM):
  Part: QSH-090-01-L-D-A / QTH-090-01-L-D-A (Samtec)
  Pitch: 0.50mm
  Positions: 90 per row (180 total dual row)
  Current: 2A per signal, 25A per ground plane
  Data rate: 25 Gbps

POWER CONNECTORS (Quad Board):
  VICOR BCM: BCM48BF120T300A00
  Test/I2C: Molex 22-05-3031
```

---

## APPENDIX B: DOCUMENT HISTORY

| Date       | Change                                              |
|------------|-----------------------------------------------------|
| 2026-02-09 | CORRECTED: Quad Board is power only, not signals    |
| 2026-02-09 | CORRECTED: QSH connects Controller directly to BIM  |
| 2026-01-13 | Initial version (contained errors)                  |

---

**END OF DOCUMENT**
