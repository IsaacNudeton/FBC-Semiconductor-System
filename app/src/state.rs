/// Global app state — boards, selection, telemetry, panel state.
/// Plain struct, mutated directly, no subscriptions.
///
/// Unified board model: FBC + Sonoma boards in a single list, no mode toggle.
/// Navigation: 4 tabs (Dashboard, Profiling, Engineering, Datalogs) + persistent sidebar board tree.

use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::mpsc;
use fbc_host::types::*;
use crate::transport::{BoardId, HwCommand, HwResponse};

const MAX_TELEMETRY: usize = 1000;

// ---- Cisco Switch State ----

/// A single switch port entry (from `show interfaces status` + `show mac address-table`)
#[derive(Clone, Debug)]
pub struct SwitchPort {
    pub port: String,           // e.g. "Gi0/1", "Fa0/12"
    pub description: String,    // port description (if configured)
    pub status: String,         // "connected", "notconnect", "disabled"
    pub vlan: String,           // VLAN number or "trunk"
    pub speed: String,          // "100", "1000", "auto"
    pub duplex: String,         // "full", "half", "auto"
    pub mac_address: String,    // learned MAC (from mac address-table)
    pub board_id: Option<BoardId>, // cross-referenced board (if MAC matches a discovered board)
}

/// Overall switch state
#[derive(Clone, Debug)]
pub struct SwitchState {
    pub connected: bool,
    pub com_port: String,       // e.g. "COM4"
    pub hostname: String,       // switch hostname from prompt
    pub ports: Vec<SwitchPort>,
    pub last_error: Option<String>,
    pub last_poll_ms: u64,
}

impl Default for SwitchState {
    fn default() -> Self {
        Self {
            connected: false,
            com_port: "COM4".into(),
            hostname: String::new(),
            ports: Vec::new(),
            last_error: None,
            last_poll_ms: 0,
        }
    }
}

/// Target for orchestration — who receives a command.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandTarget {
    /// Single selected board
    Selected,
    /// Specific board by ID
    One(BoardId),
    /// A user-selected subset (by index in boards vec)
    Set(Vec<usize>),
    /// All FBC boards
    AllFbc,
    /// All Sonoma boards
    AllSonoma,
    /// Every discovered board
    All,
}

// ---- 4-Tab Navigation ----

/// Top-level tabs (the 4 production workflow tabs)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Profiling,
    Engineering,
    Datalogs,
}

impl Tab {
    pub const ALL: &[Tab] = &[Tab::Dashboard, Tab::Profiling, Tab::Engineering, Tab::Datalogs];

    pub fn label(&self) -> &'static str {
        match self {
            Tab::Dashboard   => "Dashboard",
            Tab::Profiling   => "Profiling",
            Tab::Engineering => "Engineering",
            Tab::Datalogs    => "Datalogs",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Dashboard   => "#",
            Tab::Profiling   => "D",
            Tab::Engineering => ">",
            Tab::Datalogs    => "L",
        }
    }

    /// Sub-panels available within this tab
    pub fn sub_views(&self) -> &[View] {
        match self {
            Tab::Dashboard   => &[View::Overview, View::Facility, View::Board],
            Tab::Profiling   => &[View::Pattern, View::Device, View::TestPlan],
            Tab::Engineering => &[View::Terminal, View::Power, View::Analog, View::Vectors, View::Eeprom, View::Firmware, View::Waveform],
            Tab::Datalogs    => &[View::Datalogs],
        }
    }

    /// Default sub-view for this tab
    pub fn default_view(&self) -> View {
        self.sub_views()[0]
    }
}

/// Sub-panel views (kept from original 13 + 1 new Datalogs)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    // Dashboard sub-panels
    Overview,
    Facility,
    Board,
    // Profiling sub-panels
    Pattern,
    Device,
    TestPlan,
    // Engineering sub-panels
    Terminal,
    Power,
    Analog,
    Vectors,
    Eeprom,
    Firmware,
    Waveform,
    // Datalogs sub-panel
    Datalogs,
}

impl View {
    pub fn label(&self) -> &'static str {
        match self {
            View::Overview  => "Overview",
            View::Facility  => "Facility",
            View::Board     => "Board",
            View::Pattern   => "Pattern",
            View::Device    => "Device",
            View::TestPlan  => "Test Plan",
            View::Terminal  => "Terminal",
            View::Power     => "Power",
            View::Analog    => "Analog",
            View::Vectors   => "Vectors",
            View::Eeprom    => "EEPROM",
            View::Firmware  => "Firmware",
            View::Waveform  => "Waveform",
            View::Datalogs  => "Datalogs",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            View::Overview  => "#",
            View::Facility  => "~",
            View::Board     => "B",
            View::Pattern   => "C",
            View::Device    => "D",
            View::TestPlan  => "T",
            View::Terminal  => ">",
            View::Power     => "P",
            View::Analog    => "A",
            View::Vectors   => "V",
            View::Eeprom    => "E",
            View::Firmware  => "F",
            View::Waveform  => "W",
            View::Datalogs  => "L",
        }
    }

    /// Legacy flat list (for backward compat if needed)
    pub const ALL: &[View] = &[
        View::Overview, View::Power, View::Analog, View::Vectors,
        View::Board, View::Device, View::Terminal, View::Eeprom,
        View::TestPlan, View::Firmware, View::Pattern, View::Facility,
        View::Waveform, View::Datalogs,
    ];
}

// ---- Sidebar Board Tree ----

/// Shelf/tray/board hierarchy for sidebar display.
/// Sonoma: IP→shelf computed (101-144=front, 201-244=rear).
/// FBC: sequential assignment.
#[derive(Clone, Debug)]
pub struct ShelfSlot {
    pub shelf: u8,         // 1-11
    pub tray: &'static str, // "Front" or "Rear"
    pub position: u8,      // 1-4 within tray
}

impl ShelfSlot {
    /// Compute slot from Sonoma IP address (172.16.0.xxx)
    pub fn from_sonoma_ip(ip: &str) -> Option<Self> {
        let last_octet: u8 = ip.rsplit('.').next()?.parse().ok()?;
        if last_octet >= 101 && last_octet <= 144 {
            let idx = last_octet - 101; // 0-43
            Some(ShelfSlot {
                shelf: idx / 4 + 1,
                tray: "Front",
                position: idx % 4 + 1,
            })
        } else if last_octet >= 201 && last_octet <= 244 {
            let idx = last_octet - 201; // 0-43
            Some(ShelfSlot {
                shelf: idx / 4 + 1,
                tray: "Rear",
                position: idx % 4 + 1,
            })
        } else {
            None
        }
    }

    /// Sequential assignment for FBC boards
    pub fn from_fbc_index(idx: usize) -> Self {
        let shelf = (idx / 8) as u8 + 1;
        let tray_idx = idx % 8;
        let (tray, pos) = if tray_idx < 4 {
            ("Front", tray_idx as u8 + 1)
        } else {
            ("Rear", (tray_idx - 4) as u8 + 1)
        };
        ShelfSlot { shelf, tray, position: pos }
    }
}

/// Snapshot of telemetry at a point in time
#[derive(Clone)]
pub struct TelemetryEntry {
    pub timestamp_ms: u64,
    pub state: ControllerState,
    pub temp_c: f32,
    pub vicor_cores: Option<[VicorCore; 6]>,
    pub analog: Option<AnalogChannels>,
    pub vector_status: Option<VectorEngineStatus>,
}

/// Unified board state — works for both FBC and Sonoma boards.
/// FBC boards populate from BoardInfo (mac, serial, fw_version).
/// Sonoma boards populate from SSH probe (ip, alive, fw_version).
#[derive(Clone)]
pub struct BoardState {
    pub id: BoardId,
    pub system_type: SystemType,
    pub label: String,
    pub alive: bool,
    pub fw_version: String,
    // Common telemetry (both profiles populate these)
    pub status: Option<BoardStatus>,
    pub analog: Option<AnalogChannels>,
    pub vicor: Option<VicorStatus>,
    pub vector_status: Option<VectorEngineStatus>,
    pub eeprom_data: Option<EepromData>,
    pub fast_pins: Option<FastPinState>,
    pub error_log: Option<ErrorLogResponse>,
    pub firmware_info: Option<FirmwareInfo>,
    pub pmbus: Option<PmBusStatus>,
    // FBC-specific discovery data
    pub fbc_info: Option<fbc_host::BoardInfo>,
    // Sonoma-specific
    pub sonoma_status: Option<SonomaStatus>,
}

/// Common board status (extracted from StatusResponse or SonomaStatus)
#[derive(Clone)]
pub struct BoardStatus {
    pub state: ControllerState,
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: f32,
    pub fpga_vccint: u16,
    pub fpga_vccaux: u16,
}

impl BoardState {
    /// Create from FBC discovery (BoardInfo)
    pub fn from_fbc(info: &fbc_host::BoardInfo) -> Self {
        Self {
            id: BoardId::Mac(info.mac),
            system_type: SystemType::Fbc,
            label: fbc_host::format_mac(&info.mac),
            alive: true,
            fw_version: format!("{}.{}", info.fw_version >> 8, info.fw_version & 0xFF),
            status: None,
            analog: None,
            vicor: None,
            vector_status: None,
            eeprom_data: None,
            fast_pins: None,
            error_log: None,
            firmware_info: None,
            pmbus: None,
            fbc_info: Some(info.clone()),
            sonoma_status: None,
        }
    }

    /// Create from Sonoma SSH scan
    pub fn from_sonoma(ip: &str, alive: bool, fw_version: &str) -> Self {
        Self {
            id: BoardId::Ip(ip.to_string()),
            system_type: SystemType::Sonoma,
            label: ip.to_string(),
            alive,
            fw_version: fw_version.to_string(),
            status: None,
            analog: None,
            vicor: None,
            vector_status: None,
            eeprom_data: None,
            fast_pins: None,
            error_log: None,
            firmware_info: None,
            pmbus: None,
            fbc_info: None,
            sonoma_status: None,
        }
    }

    pub fn is_fbc(&self) -> bool {
        matches!(self.system_type, SystemType::Fbc)
    }

    pub fn is_sonoma(&self) -> bool {
        matches!(self.system_type, SystemType::Sonoma)
    }

    /// Short type label for display
    pub fn type_label(&self) -> &'static str {
        match self.system_type {
            SystemType::Fbc => "FBC",
            SystemType::Sonoma => "Sonoma",
            _ => "Other",
        }
    }
}

pub struct AppState {
    // Connection
    pub interface: String,
    pub connected: bool,

    // Unified board list
    pub boards: Vec<BoardState>,
    pub selected_board: Option<BoardId>,

    // Shelf slot mapping (board index -> physical location)
    pub board_slots: HashMap<BoardId, ShelfSlot>,

    // Telemetry ring buffers
    pub telemetry: HashMap<BoardId, VecDeque<TelemetryEntry>>,

    // Navigation — 4-tab + sub-panel
    pub active_tab: Tab,
    pub active_view: View,
    pub sidebar_collapsed: bool,

    // Sidebar tree state
    pub expanded_shelves: HashSet<u8>,   // which shelves are expanded in tree

    // Persistent tab indices for panels
    pub tab_indices: HashMap<&'static str, usize>,

    // Persistent float values for panels (zoom, scroll, etc.)
    pub panel_floats: HashMap<&'static str, f32>,

    // Scroll offsets per widget ID
    pub scroll_offsets: HashMap<u64, f32>,

    // Text input states (id -> cursor position)
    pub cursors: HashMap<u64, usize>,

    // Hardware command channel
    cmd_tx: mpsc::Sender<HwCommand>,

    // Status bar
    pub status_message: String,
    pub frame_count: u64,

    // Orchestration
    pub command_target: CommandTarget,
    pub multi_select: Vec<usize>, // indices into boards vec for Set target

    // Cisco switch
    pub switch: SwitchState,

    // Sonoma credentials
    pub sonoma_user: String,
    pub sonoma_password: String,
    pub sonoma_range_start: String,
    pub sonoma_range_end: String,
}

impl AppState {
    pub fn new(cmd_tx: mpsc::Sender<HwCommand>) -> Self {
        Self {
            interface: String::new(),
            connected: false,

            boards: Vec::new(),
            selected_board: None,
            board_slots: HashMap::new(),

            telemetry: HashMap::new(),

            active_tab: Tab::Dashboard,
            active_view: View::Overview,
            sidebar_collapsed: false,
            expanded_shelves: HashSet::new(),

            tab_indices: HashMap::new(),
            panel_floats: HashMap::new(),
            scroll_offsets: HashMap::new(),
            cursors: HashMap::new(),

            cmd_tx,

            status_message: "Ready".into(),
            frame_count: 0,

            command_target: CommandTarget::Selected,
            multi_select: Vec::new(),

            switch: SwitchState::default(),

            sonoma_user: "root".into(),
            sonoma_password: String::new(),
            sonoma_range_start: "172.16.0.101".into(),
            sonoma_range_end: "172.16.0.244".into(),
        }
    }

    /// Switch to a tab, setting the active sub-view to the tab's default
    pub fn switch_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
        self.active_view = tab.default_view();
    }

    /// Switch to a sub-view within the current tab
    pub fn switch_sub_view(&mut self, view: View) {
        self.active_view = view;
    }

    /// Rebuild shelf slot mapping after discovery
    pub fn rebuild_slots(&mut self) {
        self.board_slots.clear();
        let mut fbc_idx = 0usize;
        for board in &self.boards {
            let slot = match &board.id {
                BoardId::Ip(ip) => ShelfSlot::from_sonoma_ip(ip)
                    .unwrap_or_else(|| ShelfSlot::from_fbc_index(fbc_idx)),
                BoardId::Mac(_) => {
                    let s = ShelfSlot::from_fbc_index(fbc_idx);
                    fbc_idx += 1;
                    s
                }
            };
            self.board_slots.insert(board.id.clone(), slot);
        }
    }

    /// Get boards on a specific shelf, sorted by tray+position
    pub fn boards_on_shelf(&self, shelf: u8) -> Vec<(&BoardState, &ShelfSlot)> {
        let mut result: Vec<_> = self.boards.iter()
            .filter_map(|b| {
                self.board_slots.get(&b.id)
                    .filter(|s| s.shelf == shelf)
                    .map(|s| (b, s))
            })
            .collect();
        result.sort_by(|a, b| {
            a.1.tray.cmp(&b.1.tray).then(a.1.position.cmp(&b.1.position))
        });
        result
    }

    /// Get max shelf number in current board set
    pub fn max_shelf(&self) -> u8 {
        self.board_slots.values().map(|s| s.shelf).max().unwrap_or(0)
    }

    /// Send a hardware command (non-blocking)
    pub fn send_command(&self, cmd: HwCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    /// Handle a hardware response
    pub fn handle_response(&mut self, rsp: HwResponse) {
        match rsp {
            HwResponse::FbcBoards(infos) => {
                // Merge FBC boards — don't clobber existing Sonoma boards
                self.boards.retain(|b| !b.is_fbc());
                for info in &infos {
                    self.boards.push(BoardState::from_fbc(info));
                }
                self.connected = true;
                let fbc_count = infos.len();
                self.rebuild_slots();
                self.status_message = format!("Discovered {} FBC board(s)", fbc_count);
            }

            HwResponse::SonomaBoards(entries) => {
                // Merge Sonoma boards — don't clobber existing FBC boards
                self.boards.retain(|b| !b.is_sonoma());
                for (ip, alive, fw) in &entries {
                    self.boards.push(BoardState::from_sonoma(ip, *alive, fw));
                }
                self.rebuild_slots();
                self.status_message = format!("Found {} Sonoma board(s)", entries.len());
            }

            HwResponse::BoardStatus(id, status) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.status = Some(BoardStatus {
                        state: status.state,
                        cycles: status.cycles,
                        errors: status.errors,
                        temp_c: status.temp_c,
                        fpga_vccint: status.fpga_vccint,
                        fpga_vccaux: status.fpga_vccaux,
                    });
                }
            }

            HwResponse::SonomaStatusResp(id, sonoma_status) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.alive = sonoma_status.alive;
                    board.fw_version = sonoma_status.fw_version.clone();
                    board.sonoma_status = Some(sonoma_status);
                }
            }

            HwResponse::VicorStatus(id, vicor) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.vicor = Some(vicor);
                }
            }

            HwResponse::VectorStatus(id, vs) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.vector_status = Some(vs);
                }
            }

            HwResponse::AnalogChannels(id, analog) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.analog = Some(analog);
                }
            }

            HwResponse::EepromData(id, data) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.eeprom_data = Some(data);
                }
            }

            HwResponse::FastPins(id, pins) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.fast_pins = Some(pins);
                }
            }

            HwResponse::ErrorLog(id, log) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.error_log = Some(log);
                }
            }

            HwResponse::FirmwareInfoResp(id, info) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.firmware_info = Some(info);
                }
            }

            HwResponse::PmbusStatus(id, pmbus) => {
                if let Some(board) = self.find_board_mut(&id) {
                    board.pmbus = Some(pmbus);
                }
            }

            HwResponse::RunResult(id, result) => {
                let msg = if result.passed {
                    format!("Vectors PASSED ({} executed)", result.vectors_executed)
                } else {
                    format!("Vectors FAILED ({} errors)", result.errors)
                };
                self.status_message = msg;
                // Could store on board if needed
                let _ = id;
            }

            HwResponse::SwitchConnected(hostname) => {
                self.switch.connected = true;
                self.switch.hostname = hostname.clone();
                self.status_message = format!("Switch connected: {}", hostname);
            }

            HwResponse::SwitchDisconnected => {
                self.switch.connected = false;
                self.switch.ports.clear();
                self.status_message = "Switch disconnected".into();
            }

            HwResponse::SwitchPortMap(ports) => {
                // Cross-reference MACs with discovered boards
                self.switch.ports = ports.into_iter().map(|mut sp| {
                    if !sp.mac_address.is_empty() {
                        sp.board_id = self.boards.iter().find(|b| {
                            match &b.id {
                                BoardId::Mac(mac) => {
                                    fbc_host::format_mac(mac).to_lowercase()
                                        == sp.mac_address.to_lowercase()
                                }
                                _ => false,
                            }
                        }).map(|b| b.id.clone());
                    }
                    sp
                }).collect();
                self.status_message = format!("Switch: {} ports", self.switch.ports.len());
            }

            HwResponse::SwitchCommandResult(output) => {
                self.switch.last_error = None;
                self.status_message = format!("Switch: {}", truncate_status(&output, 80));
            }

            HwResponse::SwitchError(err) => {
                self.switch.last_error = Some(err.clone());
                self.status_message = format!("Switch error: {}", err);
            }

            HwResponse::Error(msg) => {
                self.status_message = format!("Error: {}", msg);
            }

            HwResponse::Ok(msg) => {
                self.status_message = msg;
            }
        }
    }

    /// Find a board by ID (mutable)
    fn find_board_mut(&mut self, id: &BoardId) -> Option<&mut BoardState> {
        self.boards.iter_mut().find(|b| b.id == *id)
    }

    /// Find the selected board (immutable)
    pub fn selected_board_state(&self) -> Option<&BoardState> {
        self.selected_board.as_ref()
            .and_then(|id| self.boards.iter().find(|b| b.id == *id))
    }

    /// Get the selected board's MAC (if FBC)
    pub fn selected_mac(&self) -> Option<[u8; 6]> {
        match &self.selected_board {
            Some(BoardId::Mac(mac)) => Some(*mac),
            _ => None,
        }
    }

    /// Get the selected board's IP (if Sonoma)
    pub fn selected_ip(&self) -> Option<&str> {
        match &self.selected_board {
            Some(BoardId::Ip(ip)) => Some(ip),
            _ => None,
        }
    }

    /// Get persistent tab index for a panel
    pub fn tab_index(&mut self, panel: &'static str) -> usize {
        *self.tab_indices.entry(panel).or_insert(0)
    }

    /// Set persistent tab index for a panel
    pub fn set_tab_index(&mut self, panel: &'static str, idx: usize) {
        self.tab_indices.insert(panel, idx);
    }

    /// Get persistent float value for a panel key
    pub fn get_float(&self, key: &'static str) -> Option<f32> {
        self.panel_floats.get(key).copied()
    }

    /// Set persistent float value for a panel key
    pub fn set_float(&mut self, key: &'static str, value: f32) {
        self.panel_floats.insert(key, value);
    }

    /// Get cursor for a text input by ID
    pub fn cursor(&mut self, id: u64) -> &mut usize {
        self.cursors.entry(id).or_insert(0)
    }

    /// Format a board ID for display
    pub fn board_label(id: &BoardId) -> String {
        match id {
            BoardId::Mac(mac) => fbc_host::format_mac(mac),
            BoardId::Ip(ip) => ip.clone(),
        }
    }

    // ---- Orchestration ----

    /// Resolve the current CommandTarget into a list of BoardIds.
    pub fn resolve_targets(&self) -> Vec<BoardId> {
        match &self.command_target {
            CommandTarget::Selected => {
                self.selected_board.iter().cloned().collect()
            }
            CommandTarget::One(id) => vec![id.clone()],
            CommandTarget::Set(indices) => {
                indices.iter()
                    .filter_map(|&i| self.boards.get(i).map(|b| b.id.clone()))
                    .collect()
            }
            CommandTarget::AllFbc => {
                self.boards.iter().filter(|b| b.is_fbc()).map(|b| b.id.clone()).collect()
            }
            CommandTarget::AllSonoma => {
                self.boards.iter().filter(|b| b.is_sonoma()).map(|b| b.id.clone()).collect()
            }
            CommandTarget::All => {
                self.boards.iter().map(|b| b.id.clone()).collect()
            }
        }
    }

    /// Send a command to all resolved targets. The closure creates a command for each BoardId.
    pub fn send_to_targets<F>(&self, make_cmd: F)
    where
        F: Fn(BoardId) -> HwCommand,
    {
        for id in self.resolve_targets() {
            self.send_command(make_cmd(id));
        }
    }

    /// How many boards the current target resolves to.
    pub fn target_count(&self) -> usize {
        self.resolve_targets().len()
    }

    /// Human-readable label for the current target.
    pub fn target_label(&self) -> String {
        match &self.command_target {
            CommandTarget::Selected => {
                match &self.selected_board {
                    Some(id) => format!("1 board ({})", Self::board_label(id)),
                    None => "None selected".into(),
                }
            }
            CommandTarget::One(id) => Self::board_label(id),
            CommandTarget::Set(indices) => format!("{} boards", indices.len()),
            CommandTarget::AllFbc => {
                let n = self.boards.iter().filter(|b| b.is_fbc()).count();
                format!("All FBC ({})", n)
            }
            CommandTarget::AllSonoma => {
                let n = self.boards.iter().filter(|b| b.is_sonoma()).count();
                format!("All Sonoma ({})", n)
            }
            CommandTarget::All => format!("All ({})", self.boards.len()),
        }
    }

    /// Cross-reference switch ports with discovered boards (call after discovery or switch poll)
    pub fn crossref_switch_boards(&mut self) {
        for port in &mut self.switch.ports {
            if !port.mac_address.is_empty() {
                port.board_id = self.boards.iter().find(|b| {
                    match &b.id {
                        BoardId::Mac(mac) => {
                            fbc_host::format_mac(mac).to_lowercase()
                                == port.mac_address.to_lowercase()
                        }
                        _ => false,
                    }
                }).map(|b| b.id.clone());
            }
        }
    }

    /// Toggle a board index in multi_select.
    pub fn toggle_multi_select(&mut self, idx: usize) {
        if let Some(pos) = self.multi_select.iter().position(|&i| i == idx) {
            self.multi_select.remove(pos);
        } else {
            self.multi_select.push(idx);
        }
        self.command_target = CommandTarget::Set(self.multi_select.clone());
    }
}

fn truncate_status(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or(s);
    if line.len() <= max { line.to_string() } else { format!("{}...", &line[..max - 3]) }
}
