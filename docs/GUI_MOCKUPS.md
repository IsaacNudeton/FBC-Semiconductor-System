# GUI Visual Mockups — FBC Semiconductor System

Production burn-in GUI. 4 tabs + persistent sidebar. Socket = C1/C2/C3/C4.

---

## Master Layout

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│  FBC Burn-In Control System                          ▲ 3 alerts   isaac@ise  23:50 │
├────────────────┬──── [Dashboard] [Profiling] [Engineering] [Datalogs] ──────────────┤
│                │                                                                    │
│   S I D E B A R│                    C O N T E N T   A R E A                         │
│   (always      │                                                                    │
│    visible)    │     Changes based on:                                              │
│                │       • Active tab (top)                                           │
│   ~200px wide  │       • Selected entity (sidebar)                                 │
│                │                                                                    │
│                │     Sidebar selection + Tab = unique view                          │
│                │                                                                    │
│                │                                                                    │
│                │                                                                    │
│                │                                                                    │
│                │                                                                    │
│                │                                                                    │
├────────────────┴────────────────────────────────────────────────────────────────────┤
│  STATUS BAR: Connected: 44 boards │ LOT #2847 running (142h/168h) │ Temp OK        │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Sidebar (Persistent — All Tabs)

```
┌────────────────┐
│ ▼ System       │  ← Collapsible tree
│   ▼ Shelf 1    │
│     ▼ Front    │  ← Tray (front/rear)
│       ● B1     │  ● = running (green)
│       ● B2     │
│       ◉ B3     │  ◉ = error (red, pulses)
│       ● B4     │
│     ▶ Rear     │  ▶ = collapsed
│   ▶ Shelf 2    │
│   ▼ Shelf 3    │
│     ▼ Front    │
│       ○ B9     │  ○ = idle (gray)
│       ○ B10    │
│       ⊘ B11    │  ⊘ = lost connection
│       ○ B12    │
│     ▶ Rear     │
│   ▶ Shelf 4    │
│   ▶ Shelf 5    │
│   ...          │
│   ▶ Shelf 11   │
│                │
│ ─────────────  │  ← Divider
│ ACTIVE LOTS    │
│  🔵 #2847 Cisco│  ← Click = filter to LOT boards
│     C512  8/8  │     "8/8" = 8 boards assigned
│  🟢 #2848 MSFT│
│     Nrmdy 4/4  │
│  ⚪ Unassigned │  ← Boards not in any LOT
│     12 boards  │
│                │
│ ─────────────  │
│ ⚠ ALERTS (3)   │  ← Always visible
│  B3: err @142h │
│  B11: no comms │
│  B22: temp 129°│
└────────────────┘
```

**Interactions:**
- Click board → selects it, content area updates
- Click shelf → shows shelf overview in content area
- Click LOT → highlights all LOT boards in tree, shows LOT view
- Right-click board → context menu (ping, reboot, remove from LOT)
- Drag board into LOT → assign
- Alert click → jumps to Engineering tab for that board

---

## Tab 1: Dashboard — System Overview (No Board Selected)

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ System       │  DASHBOARD                                          [Load LOT ▼]  │
│   ▼ Shelf 1    │                                                                    │
│     ...        │  ┌─── System Health ──────────────────────────────────────────────┐ │
│   ▼ Shelf 2    │  │  44 boards online    2 errors    0 lost    38 idle            │ │
│     ...        │  │  ████████████████████████████████████░░░░░░░░░░░░░░░░  9% run  │ │
│   ...          │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│ ACTIVE LOTS    │  ┌─── Shelf Grid ─────────────────────────────────────────────────┐ │
│  🔵 #2847      │  │                                                                │ │
│  🟢 #2848      │  │  Shelf 1          Shelf 2          Shelf 3          Shelf 4    │ │
│  ⚪ Unassigned  │  │  ┌──┬──┬──┬──┐   ┌──┬──┬──┬──┐   ┌──┬──┬──┬──┐   ┌──┬──┬──┬──┐│ │
│                │  │  │●●│●●│◉●│●●│   │●●│●●│●●│●●│   │○○│○○│⊘○│○○│   │  │  │  │  ││ │
│ ⚠ ALERTS (3)   │  │  ├──┼──┼──┼──┤   ├──┼──┼──┼──┤   ├──┼──┼──┼──┤   ├──┼──┼──┼──┤│ │
│  ...           │  │  │●●│●●│●●│●●│   │●●│●●│●●│  │   │○○│○○│○○│○○│   │  │  │  │  ││ │
│                │  │  └──┴──┴──┴──┘   └──┴──┴──┴──┘   └──┴──┴──┴──┘   └──┴──┴──┴──┘│ │
│                │  │  Front  Rear     Front  Rear     Front  Rear     Front  Rear    │ │
│                │  │  LOT #2847       LOT #2847       (idle)          (empty)        │ │
│                │  │                                                                │ │
│                │  │  Shelf 5          Shelf 6          ...                          │ │
│                │  │  ┌──┬──┬──┬──┐   ┌──┬──┬──┬──┐                                 │ │
│                │  │  │  │  │  │  │   │  │  │  │  │                                 │ │
│                │  │  ...                                                            │ │
│                │  └────────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Active Runs ────────────────────────────────────────────────┐ │
│                │  │  LOT     Customer   Device    Boards  Progress   Time Left     │ │
│                │  │  #2847   Cisco      C512      8       ████░ 84%  26h remaining │ │
│                │  │  #2848   Microsoft  Normandy  4       ██░░░ 41%  99h remaining │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Recent Events ─────────────────────────────────────────────┐ │
│                │  │  23:47  B3 Shelf1/Front — Error: pin 47 stuck-at-0 @ loop 847 │ │
│                │  │  23:45  B11 Shelf3/Front — Connection lost (no response 30s)   │ │
│                │  │  22:10  LOT #2848 loaded — 4 boards, 168h HTOL                │ │
│                │  │  21:30  System scan — 44/44 boards discovered                  │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 1: Dashboard — Shelf Drilldown (Click Shelf 1)

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ System       │  DASHBOARD › Shelf 1                            [◀ System] [Load] │
│   ▼ Shelf 1 ◀──│                                                                    │
│     ▼ Front    │  ┌─── Front Tray ─────────────────────────────────────────────────┐ │
│       ● B1     │  │                                                                │ │
│       ● B2     │  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────┐ │ │
│       ◉ B3     │  │   │   Board 1   │  │   Board 2   │  │   Board 3   │  │  B4  │ │ │
│       ● B4     │  │   │   ● RUN     │  │   ● RUN     │  │   ◉ ERROR   │  │ ● RUN│ │ │
│     ▼ Rear     │  │   │             │  │             │  │             │  │      │ │ │
│       ● B5     │  │   │ C1: SN-4821 │  │ C1: SN-4825 │  │ C1: SN-4829 │  │ C1:  │ │ │
│       ● B6     │  │   │ C2: SN-4822 │  │ C2: SN-4826 │  │ C2: SN-4830 │  │ C2:  │ │ │
│       ● B7     │  │   │ C3: SN-4823 │  │ C3: SN-4827 │  │ C3: ─empty─ │  │ C3:  │ │ │
│       ● B8     │  │   │ C4: SN-4824 │  │ C4: SN-4828 │  │ C4: SN-4831 │  │ C4:  │ │ │
│                │  │   │             │  │             │  │             │  │      │ │ │
│ ACTIVE LOTS    │  │   │ 142h / 168h │  │ 142h / 168h │  │ ERR @ 141h  │  │142/168│ │ │
│  🔵 #2847      │  │   │ 32°C  0 err │  │ 31°C  0 err │  │ 33°C  1 err │  │31°C  │ │ │
│                │  │   └─────────────┘  └─────────────┘  └─────────────┘  └──────┘ │ │
│                │  │   BIM: Full (BIM#0042)            LOT: #2847 Cisco C512       │ │
│ ⚠ ALERTS (3)   │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Rear Tray ──────────────────────────────────────────────────┐ │
│                │  │                                                                │ │
│                │  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────┐ │ │
│                │  │   │   Board 5   │  │   Board 6   │  │   Board 7   │  │  B8  │ │ │
│                │  │   │   ● RUN     │  │   ● RUN     │  │   ● RUN     │  │ ● RUN│ │ │
│                │  │   │ C1: SN-4832 │  │ C1: SN-4836 │  │ C1: SN-4840 │  │ C1:  │ │ │
│                │  │   │ C2: SN-4833 │  │ C2: SN-4837 │  │ C2: SN-4841 │  │ C2:  │ │ │
│                │  │   │ C3: SN-4834 │  │ C3: SN-4838 │  │ C3: SN-4842 │  │ C3:  │ │ │
│                │  │   │ C4: SN-4835 │  │ C4: SN-4839 │  │ C4: SN-4843 │  │ C4:  │ │ │
│                │  │   │ 142h / 168h │  │ 142h / 168h │  │ 142h / 168h │  │142/168│ │ │
│                │  │   │ 30°C  0 err │  │ 31°C  0 err │  │ 29°C  0 err │  │30°C  │ │ │
│                │  │   └─────────────┘  └─────────────┘  └─────────────┘  └──────┘ │ │
│                │  │   BIM: Full (BIM#0043)            LOT: #2847 Cisco C512       │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  Shelf 1 Summary: 8 boards, 31 DUTs (1 empty socket), 142h/168h   │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 1: Dashboard — Board Drilldown (Click Board 3)

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ Shelf 1      │  DASHBOARD › Shelf 1 › Front › Board 3       [◀ Shelf] [Eng ▶]   │
│   ▼ Front      │                                                                    │
│     ● B1       │  ┌─── Board 3 ── ◉ ERROR ── LOT #2847 ──────────────────────────┐ │
│     ● B2       │  │                                                                │ │
│     ◉ B3  ◀────│  │  BIM: Full #0042  │  FW: v2.1.0  │  Profile: FBC              │ │
│     ● B4       │  │  MAC: 00:0A:35:AD:12:7F  │  Uptime: 141h 23m                  │ │
│   ▶ Rear       │  │                                                                │ │
│                │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │ │
│                │  │  │    C1    │  │    C2    │  │    C3    │  │    C4    │       │ │
│                │  │  │ SN-4829  │  │ SN-4830  │  │  EMPTY   │  │ SN-4831  │       │ │
│ ACTIVE LOTS    │  │  │  ● PASS  │  │  ● PASS  │  │  ○ ───   │  │  ◉ FAIL  │       │ │
│  🔵 #2847      │  │  │ 0 errors │  │ 0 errors │  │ (marked) │  │ 1 error  │       │ │
│                │  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘       │ │
│                │  │                                                                │ │
│ ⚠ ALERTS (3)   │  │  ERROR DETAILS (C4):                                           │ │
│  B3: err @142h │  │  ┌────────────────────────────────────────────────────────┐    │ │
│                │  │  │  Vector 12,847  Loop 3/1000  Pin 47  stuck-at-0       │    │ │
│                │  │  │  Expected: H  Got: L  Time: 141h 12m 34s              │    │ │
│                │  │  │  [View in Engineering ▶]  [View Datalog ▶]            │    │ │
│                │  │  └────────────────────────────────────────────────────────┘    │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Live Telemetry ────────────────────────────────────────────┐ │
│                │  │                                                                │ │
│                │  │  Temperature          Voltage (VICOR)        Current           │ │
│                │  │  Case: 33.2°C         Core1: 0.800V ✓       Core1: 1.24A      │ │
│                │  │  DUT:  41.7°C         Core2: 1.100V ✓       Core2: 0.89A      │ │
│                │  │  Die:  45.1°C         Core3: 1.800V ✓       Core3: 2.31A      │ │
│                │  │                       Core4: 0.750V ✓       Core4: 0.42A      │ │
│                │  │  ▁▂▃▄▅▅▅▅▅▅▅▅▄▄▅    Core5: 3.300V ✓       Core5: 0.11A      │ │
│                │  │  (24h temp trend)     Core6: 1.200V ✓       Core6: 0.67A      │ │
│                │  │                                                                │ │
│                │  │  Vectors: 12,847/102 × Loop 3/1000     ████████████░░ 84.5%   │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 1: Dashboard — LOT Loader Wizard

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ System       │  LOAD LOT                                          Step 2 of 5    │
│   ...          │                                                                    │
│                │  ┌─── LOT Information ───────────────────────────────────────────┐ │
│                │  │                                                                │ │
│ ACTIVE LOTS    │  │  LOT #:     [2849        ]     (or scan barcode)              │ │
│  🔵 #2847      │  │  Customer:  [Intel       ▼]                                   │ │
│  🟢 #2848      │  │  Device:    [Raptor Lake ▼]                                   │ │
│  ⚪ Unassigned  │  │  Project #: [RL-2026-041 ]                                   │ │
│                │  │  Units:     [32          ]     (DUTs to burn in)              │ │
│                │  │  Run Type:  [HTOL        ▼]    (HTOL / EFR / Qual / Custom)  │ │
│                │  │  Duration:  [168h        ▼]    (168h / 500h / 1000h / Custom)│ │
│ ⚠ ALERTS (3)   │  │  Test Spec: [RL-HTOL-v3  ]    (from Device Profile)          │ │
│                │  │                                                                │ │
│                │  │  ✓ EEPROM check: 8 boards with BIM# available                 │ │
│                │  │  ✓ 32 units → need 8 boards (4 sockets each)                  │ │
│                │  │                                                                │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Board Assignment ──────────────────────────────────────────┐ │
│                │  │                                                                │ │
│                │  │  Auto-assigned to Shelf 4 (8 boards available):               │ │
│                │  │                                                                │ │
│                │  │  ┌─── Shelf 4 / Front ───────────────────────────────┐        │ │
│                │  │  │  B13        B14        B15        B16            │        │ │
│                │  │  │  C1: [    ] C1: [    ] C1: [    ] C1: [    ]     │        │ │
│                │  │  │  C2: [    ] C2: [    ] C2: [    ] C2: [    ]     │        │ │
│                │  │  │  C3: [    ] C3: [    ] C3: [    ] C3: [    ]     │        │ │
│                │  │  │  C4: [    ] C4: [    ] C4: [    ] C4: [    ]     │        │ │
│                │  │  └───────────────────────────────────────────────────┘        │ │
│                │  │  ┌─── Shelf 4 / Rear ────────────────────────────────┐        │ │
│                │  │  │  B17        B18        B19        B20            │        │ │
│                │  │  │  C1: [    ] C1: [    ] C1: [    ] C1: [    ]     │        │ │
│                │  │  │  C2: [    ] C2: [    ] C2: [    ] C2: [    ]     │        │ │
│                │  │  │  C3: [    ] C3: [    ] C3: [    ] C3: [    ]     │        │ │
│                │  │  │  C4: [    ] C4: [    ] C4: [    ] C4: [    ]     │        │ │
│                │  │  └───────────────────────────────────────────────────┘        │ │
│                │  │                                                                │ │
│                │  │  [Mark socket empty]  [Scan serial #]  [Paste serials from CSV]│ │
│                │  │                                                                │ │
│                │  │  32/32 sockets filled    0 empty    0 damaged                  │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │                        [◀ Back]   [Next: Verify ▶]                │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 3: Engineering — Board Selected (Board 3)

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ Shelf 1      │  ENGINEERING › Board 3 (FBC)                   [◀ Dashboard]       │
│   ▼ Front      │                                                                    │
│     ◉ B3  ◀────│  ┌─── Command Palette ──────────────────────────────── Ctrl+K ──┐ │
│                │  │  > power _                                                    │ │
│                │  │                                                                │ │
│                │  │  Suggestions:                                                  │ │
│                │  │    power on VOUT1     → Core1 @ 0.800V (safe default)         │ │
│                │  │    power on VOUT2     → Core2 @ 1.100V (safe default)         │ │
│                │  │    power off all      → Sequence off (reverse order, safe)    │ │
│                │  │    power status       → Read all 6 VICOR rails               │ │
│                │  │    power set VOUT3 1.5V  ⚠ Max rated: 1.85V                  │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│ ACTIVE LOTS    │  ┌─── Quick Actions ────────────────────────────────────────────┐ │
│  🔵 #2847      │  │                                                                │ │
│                │  │  [██ EMERGENCY STOP ██]   [Power On]  [Power Off]  [Refresh]  │ │
│                │  │                                                                │ │
│ ⚠ ALERTS (3)   │  │  [Upload Vectors]  [Start]  [Pause]  [Resume]  [Read Analog] │ │
│                │  │                                                                │ │
│                │  │  [Read EEPROM]  [Heater On]  [Heater Off]  [Set Temp: 125°C]  │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Terminal ──────────────────────────────────────────────────┐ │
│                │  │  Board 3 │ FBC │ MAC 00:0A:35:AD:12:7F                        │ │
│                │  │                                                                │ │
│                │  │  00:14  > power on VOUT1 0.8                                  │ │
│                │  │  00:14    ✓ Core1 enabled @ 0.800V                            │ │
│                │  │  00:14  > power on VOUT2 1.1                                  │ │
│                │  │  00:14    ✓ Core2 enabled @ 1.100V                            │ │
│                │  │  00:15  > read analog all                                     │ │
│                │  │  00:15    ch[0-7]:  1201 1198 1204 1199 1202 1197 1200 1203  │ │
│                │  │           ch[8-15]: 800  1100 1800 750  3300 1200 0    0     │ │
│                │  │           ch[16-23]: ...                                      │ │
│                │  │  00:16  > heater on C2 125                                    │ │
│                │  │  00:16    ✓ Heater C2 target 125.0°C (auto-shutoff @ 135°C)  │ │
│                │  │  00:16    ⚠ Auto-cooling enabled if case > 130°C              │ │
│                │  │  00:18  > vectors upload bringup_fast_pins.fbc                │ │
│                │  │  00:18    ✓ Uploaded 102 vectors (1,847 bytes) via DMA        │ │
│                │  │  00:18  > vectors start 1000                                  │ │
│                │  │  00:18    ✓ Vector engine started, 1000 loops                 │ │
│                │  │                                                                │ │
│                │  │  > _                                                           │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Live Monitor (auto-refresh 2s) ───────────────────────────┐ │
│                │  │  Temp: Case 33°C  DUT 42°C  Die 45°C   Vector: 847/102 ×L3  │ │
│                │  │  VICOR: 0.80 1.10 1.80 0.75 3.30 1.20  Errors: 1            │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 3: Engineering — BIM Visualization

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ Shelf 1      │  ENGINEERING › Board 3 › BIM View                                 │
│   ▼ Front      │                                                                    │
│     ◉ B3  ◀────│  ┌─── BIM #0042 (Full) ────────────────────────────────────────┐  │
│                │  │                                                               │  │
│                │  │          ┌──────────────────────────────────────┐             │  │
│                │  │          │          CONTROLLER BOARD             │             │  │
│                │  │          │          (Zynq 7020)                  │             │  │
│                │  │          │                                       │             │  │
│                │  │          │  [FPGA]    [ARM]    [DDR]    [ETH]   │             │  │
│                │  │          │  45°C      42°C     --       ● Link  │             │  │
│                │  │          │                                       │             │  │
│                │  │          │  J3 ─────────────────┐               │             │  │
│                │  │          └──┬───────────────────┼───────────────┘             │  │
│                │  │             │                   │                              │  │
│                │  │     ┌───────┴───────────────────┴──────────────────┐          │  │
│                │  │     │              BIM (INTERPOSER)                 │          │  │
│                │  │     │                                               │          │  │
│                │  │     │  ┌─────────┐  ┌─────────┐  ┌────┐  ┌─────┐ │          │  │
│                │  │     │  │   C1    │  │   C2    │  │ C3 │  │ C4  │ │          │  │
│                │  │     │  │ SN-4829 │  │ SN-4830 │  │EMTY│  │4831 │ │          │  │
│                │  │     │  │ ● PASS  │  │ ● PASS  │  │ ── │  │◉FAIL│ │          │  │
│                │  │     │  │ 33.2°C  │  │ 31.8°C  │  │    │  │34.1°│ │          │  │
│                │  │     │  └─────────┘  └─────────┘  └────┘  └─────┘ │          │  │
│                │  │     │                                               │          │  │
│                │  │     │  [EEPROM ✓]  [NTC]  [HEATER FET]  [FAN FET] │          │  │
│                │  │     │  BIM#0042    30kΩ   STD16NF06LT4   ON       │          │  │
│                │  │     └───────────────────────────────────────────────┘          │  │
│                │  │                                                               │  │
│                │  │  VICOR Rails:  V1=0.80V ✓  V2=1.10V ✓  V3=1.80V ✓           │  │
│                │  │                V4=0.75V ✓  V5=3.30V ✓  V6=1.20V ✓           │  │
│                │  │                                                               │  │
│                │  │  128 BIM pins (2-cycle)  │  32 fast pins (1-cycle)            │  │
│                │  │  ████████████████████████  ████████                           │  │
│                │  │  (IO: 96 drive, 32 mon)   (all output in test)               │  │
│                │  └──────────────────────────────────────────────────────────────┘  │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 4: Datalogs — LOT Summary

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ System       │  DATALOGS                                    [Export ▼] [Filter ▼] │
│   ...          │                                                                    │
│                │  ┌─── LOT #2847 — Cisco C512 ───────────────────────────────────┐ │
│ ACTIVE LOTS    │  │                                                                │ │
│  🔵 #2847 ◀────│  │  Status: RUNNING (84.5%)     Start: Mar 19 08:00              │ │
│  🟢 #2848      │  │  Boards: 8 (32 sockets)      ETA:   Mar 26 08:00              │ │
│  ⚪ Unassigned  │  │  Pass: 30/32  Fail: 1/32  Empty: 1/32                         │ │
│                │  │                                                                │ │
│                │  │  ┌── Temperature Profile (all boards, 7 days) ──────────────┐ │ │
│                │  │  │  135°─                                                    │ │ │
│                │  │  │  130°─         ╭──────────────────────────────────╮       │ │ │
│                │  │  │  125°─────────╯                                    ╰───── │ │ │
│                │  │  │  120°─                                                    │ │ │
│                │  │  │       ├──────┼──────┼──────┼──────┼──────┼──────┼──────┤ │ │ │
│                │  │  │      Day1   Day2   Day3   Day4   Day5   Day6   Day7    │ │ │
│                │  │  │  — avg  ─── B3 (outlier)                              │ │ │
│                │  │  └──────────────────────────────────────────────────────────┘ │ │
│                │  │                                                                │ │
│                │  │  ⚠ Anomalies Detected:                                         │ │
│                │  │    • B3 ran 3.1°C above fleet avg (33.2°C vs 30.1°C)          │ │
│                │  │    • B3-C4: pin 47 stuck-at-0 at vector 12,847 loop 3         │ │
│                │  │                                                                │ │
│                │  │  ┌── Board Results ─────────────────────────────────────────┐ │ │
│                │  │  │  Board  Sockets  Pass  Fail  Errors  Avg Temp  Status   │ │ │
│                │  │  │  B1     4/4      4     0     0       30.1°C    ● RUN    │ │ │
│                │  │  │  B2     4/4      4     0     0       30.4°C    ● RUN    │ │ │
│                │  │  │  B3     3/4      2     1     1       33.2°C    ◉ ERR    │ │ │
│                │  │  │  B4     4/4      4     0     0       29.8°C    ● RUN    │ │ │
│                │  │  │  B5     4/4      4     0     0       30.0°C    ● RUN    │ │ │
│                │  │  │  B6     4/4      4     0     0       31.1°C    ● RUN    │ │ │
│                │  │  │  B7     4/4      4     0     0       29.5°C    ● RUN    │ │ │
│                │  │  │  B8     4/4      4     0     0       30.3°C    ● RUN    │ │ │
│                │  │  └─────────────────────────────────────────────────────────┘ │ │
│                │  │                                                                │ │
│                │  │  [Export PDF]  [Export CSV]  [Push to LRM]  [Re-run B3-C4]     │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── All LOTs ──────────────────────────────────────────────────┐ │
│                │  │  LOT     Customer    Device     Date       Status    Pass%    │ │
│                │  │  #2847   Cisco       C512       Mar 19     Running   93.8%   │ │
│                │  │  #2848   Microsoft   Normandy   Mar 20     Running   100%    │ │
│                │  │  #2845   AMD         Zen5       Mar 12     Complete  100%    │ │
│                │  │  #2843   Intel       RL-14      Mar 05     Complete  97.2%   │ │
│                │  │  #2840   Cisco       C256       Feb 28     Complete  100%    │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 4: Datalogs — Customer Export Preview

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│                │  EXPORT PREVIEW — LOT #2847                    [← Back] [Send ▶]  │
│                │                                                                    │
│                │  ┌────────────────────────────────────────────────────────────────┐│
│                │  │                                                                ││
│                │  │              ISE LABS — BURN-IN TEST REPORT                    ││
│                │  │              ════════════════════════════════                   ││
│                │  │                                                                ││
│                │  │  Customer:    Cisco Systems                                    ││
│                │  │  Device:      C512 (14nm FinFET)                               ││
│                │  │  LOT #:       2847                                             ││
│                │  │  Project:     C512-HTOL-2026                                   ││
│                │  │  Test Type:   HTOL (High Temperature Operating Life)           ││
│                │  │  Duration:    168 hours (7 days)                               ││
│                │  │  Start:       2026-03-19 08:00:00                              ││
│                │  │  End:         2026-03-26 08:00:00 (est.)                       ││
│                │  │                                                                ││
│                │  │  RESULTS SUMMARY                                               ││
│                │  │  ─────────────                                                 ││
│                │  │  Total Units:     32                                           ││
│                │  │  Pass:            30 (93.75%)                                  ││
│                │  │  Fail:            1  (3.12%)                                   ││
│                │  │  Empty Socket:    1  (excluded)                                ││
│                │  │                                                                ││
│                │  │  FAILURE DETAILS                                               ││
│                │  │  ───────────────                                               ││
│                │  │  Unit SN-4831 (Board 3, Socket C4):                            ││
│                │  │    Failure Mode: Pin 47 stuck-at-0                             ││
│                │  │    First Occurrence: Vector 12,847 / Loop 3                    ││
│                │  │    Time: 141h 12m 34s                                          ││
│                │  │    Temperature at failure: 33.2°C (within spec)                ││
│                │  │                                                                ││
│                │  │  ENVIRONMENTAL DATA                                            ││
│                │  │  ────────────────────                                          ││
│                │  │  Temperature: 125.0°C ± 0.3°C (setpoint: 125°C)              ││
│                │  │  Voltage Stability: ±0.3% (all rails within spec)             ││
│                │  │  [Temperature graph]  [Voltage graph]  [Error timeline]        ││
│                │  │                                                                ││
│                │  │  ─────────────────────────────────────────────                 ││
│                │  │  Generated by FBC Burn-In System v2.1                          ││
│                │  │  ISE Labs, Fremont CA                                          ││
│                │  │                                                                ││
│                │  └────────────────────────────────────────────────────────────────┘│
│                │                                                                    │
│                │  Format: [PDF ▼]   Include: [✓ Graphs] [✓ Raw Data] [✓ Photos]   │
│                │                                                                    │
│                │  [Download]  [Email to Customer]  [Push to LRM v2]                │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Tab 2: Device Profiling — Wizard (Step 4: Vectors)

```
┌────────────────┬────────────────────────────────────────────────────────────────────┐
│ ▼ System       │  DEVICE PROFILING › Intel Raptor Lake          Step 4 of 6        │
│   ...          │  ○ Device  ○ Pins  ○ Power  ● Vectors  ○ Test Plan  ○ Review      │
│                │                                                                    │
│                │  ┌─── Vector Source ─────────────────────────────────────────────┐ │
│                │  │                                                                │ │
│                │  │  Input:  [raptor_lake_htol.stil          ] [Browse]            │ │
│                │  │  Format: STIL (auto-detected)  │  Signals: 156  │  Vectors: 48K│ │
│                │  │  PIN_MAP: [RL_PIN_MAP.txt                ] [Browse]            │ │
│                │  │                                                                │ │
│                │  │  Output Format:  (● .fbc (FBC compressed)  ○ .hex (Sonoma)    │ │
│                │  │  Clock: [100 MHz ▼]                                            │ │
│                │  │                                                                │ │
│                │  │  [Convert ▶]                                                   │ │
│                │  │                                                                │ │
│                │  │  ✓ Converted: 48,192 vectors → 12,847 bytes .fbc (74.2x comp) │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │  ┌─── Vector Preview ───────────────────────────────────────────┐ │
│                │  │                                                                │ │
│                │  │  Pin Activity Heatmap (160 pins × 48K vectors):               │ │
│                │  │                                                                │ │
│                │  │  Pin 0   ░░░░████░░████░░░░████░░████░░░░████░░████  Toggle:45%│ │
│                │  │  Pin 1   ████████████████████████████████████████████  Toggle:98%│ │
│                │  │  Pin 2   ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Monitor   │ │
│                │  │  ...                                                           │ │
│                │  │  Pin 47  ████████████████████████████████░░░░░░░░░░  Toggle:72%│ │
│                │  │  ...                                                           │ │
│                │  │  Pin 159 ░░░░████████░░░░████████░░░░████████░░░░██  Toggle:50%│ │
│                │  │                                                                │ │
│                │  │  ┌── Thermal Profile (from compile-time analysis) ──────────┐ │ │
│                │  │  │  Segment   Vectors      Toggle Rate   Power Level        │ │ │
│                │  │  │  0         0-1023       34/160 (21%)  Low                │ │ │
│                │  │  │  1         1024-2047    89/160 (56%)  Medium             │ │ │
│                │  │  │  2         2048-3071    142/160 (89%) High   ← peak     │ │ │
│                │  │  │  3         3072-4095    67/160 (42%)  Medium             │ │ │
│                │  │  │  ...       ...          ...           ...                │ │ │
│                │  │  │  47        47104-48191  45/160 (28%)  Low                │ │ │
│                │  │  └─────────────────────────────────────────────────────────┘ │ │
│                │  │                                                                │ │
│                │  │  Estimated power draw: 2.1W avg, 4.8W peak (segment 2)        │ │
│                │  └───────────────────────────────────────────────────────────────┘ │
│                │                                                                    │
│                │                        [◀ Power + Timing]   [Test Plan ▶]         │
└────────────────┴────────────────────────────────────────────────────────────────────┘
```

---

## Navigation Flow Summary

```
                         SIDEBAR CLICK                    TAB CLICK
                              │                               │
              ┌───────────────┼───────────────┐               │
              ▼               ▼               ▼               ▼
          System          Shelf 3          Board 7         Tab changes
              │               │               │               │
    ┌─────────┼─────────┐     │      ┌────────┼────────┐     │
    ▼         ▼         ▼     ▼      ▼        ▼        ▼     ▼
Dashboard  Profiling  Eng   Logs  Dashboard Profiling  Eng   Content updates
    │         │        │     │       │         │        │     for selected
    ▼         ▼        ▼     ▼       ▼         ▼        ▼     entity
 System    (same)  (same) All    Shelf      Board     Board
 overview          LOTs   overview config    config
 shelf grid

 Key: sidebar selection + active tab = unique content view
      Every combination is meaningful
```

---

## Socket Notation Reference

```
Full BIM (4 boards × 4 sockets = 16 DUTs max):

  Board 1              Board 2              Board 3              Board 4
  ┌──┬──┬──┬──┐       ┌──┬──┬──┬──┐       ┌──┬──┬──┬──┐       ┌──┬──┬──┬──┐
  │C1│C2│C3│C4│       │C1│C2│C3│C4│       │C1│C2│C3│C4│       │C1│C2│C3│C4│
  └──┴──┴──┴──┘       └──┴──┴──┴──┘       └──┴──┴──┴──┘       └──┴──┴──┴──┘

Half BIM (2 boards × 4 sockets = 8 DUTs max):

  Board 1              Board 2
  ┌──┬──┬──┬──┐       ┌──┬──┬──┬──┐
  │C1│C2│C3│C4│       │C1│C2│C3│C4│
  └──┴──┴──┴──┘       └──┴──┴──┴──┘

Socket states:
  ● = DUT present, passing
  ◉ = DUT present, failing
  ○ = DUT present, idle
  ⊘ = Empty (marked by user)
  ✕ = Damaged (marked by user, excluded from count)
```
