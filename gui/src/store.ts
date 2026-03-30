import { create } from 'zustand'

export interface BoardInfo {
  mac: string
  serial: number
  hw_revision: number
  fw_version: string
  has_bim: boolean
  bim_type: number
  state: 'idle' | 'running' | 'done' | 'error' | 'unknown'
  slot?: SlotPosition
}

// Live board state from realtime monitor (matches Rust LiveBoardState)
export interface LiveBoardState {
  mac: string
  position: SlotPosition | null
  online: boolean
  last_seen: number  // Unix timestamp ms
  first_seen: number
  state: string      // "idle", "running", "error", "done"
  cycles: number
  errors: number
  temp_c: number
  run_time_ms: number
  uptime_ms: number
}

// Board event from backend (matches Rust BoardEvent)
export type BoardEvent =
  | { type: 'connected'; mac: string; position: SlotPosition | null }
  | { type: 'disconnected'; mac: string }
  | { type: 'state-changed'; mac: string; old_state: string; new_state: string }
  | { type: 'error'; mac: string; error_count: number }
  | { type: 'heartbeat'; mac: string; state: LiveBoardState }

// Historical telemetry entry (stored per board)
export interface TelemetryEntry {
  timestamp: number      // Unix timestamp ms
  cycles: number
  errors: number
  temp_c: number
  state: string
  fpga_vccint_mv?: number
  fpga_vccaux_mv?: number
}

// Test run summary (completed test)
export interface TestRunSummary {
  start_time: number
  end_time: number
  total_cycles: number
  total_errors: number
  final_state: string
  peak_temp_c: number
  avg_temp_c: number
}

export interface SlotPosition {
  shelf: number
  tray: 'front' | 'back'
  slot: number
}

export interface RackConfig {
  shelves: number
  boards_per_tray: number
  dual_tray: boolean
  assignments: { mac: string; position: SlotPosition }[]
}

// Sonoma board info from scan
export interface SonomaBoardInfo {
  ip: string
  alive: boolean
  hostname: string
}

// Maximum entries to keep in history per board
const MAX_HISTORY_ENTRIES = 1000

// Control mode: which backend protocol to use
export type ControlMode = 'fbc' | 'sonoma'

interface AppState {
  // Connection
  connected: boolean
  currentInterface: string | null
  interfaces: string[]

  // Control mode (FBC raw Ethernet vs Sonoma SSH)
  controlMode: ControlMode
  sonomaBoards: SonomaBoardInfo[]
  selectedSonomaBoard: string | null  // IP address

  // Boards (legacy - from discover)
  boards: BoardInfo[]
  selectedBoard: string | null

  // Live boards (from realtime monitor - auto-detected)
  liveBoards: Map<string, LiveBoardState>

  // Configuration
  rackConfig: RackConfig

  // Telemetry History (keyed by MAC address)
  telemetryHistory: Map<string, TelemetryEntry[]>
  testRuns: Map<string, TestRunSummary[]>

  // Actions
  setConnected: (connected: boolean) => void
  setCurrentInterface: (iface: string | null) => void
  setInterfaces: (interfaces: string[]) => void
  setBoards: (boards: BoardInfo[]) => void
  setSelectedBoard: (mac: string | null) => void
  setRackConfig: (config: RackConfig) => void
  getBoardAtPosition: (shelf: number, tray: 'front' | 'back', slot: number) => BoardInfo | null

  // Sonoma actions
  setControlMode: (mode: ControlMode) => void
  setSonomaBoards: (boards: SonomaBoardInfo[]) => void
  setSelectedSonomaBoard: (ip: string | null) => void

  // Live board actions
  setLiveBoards: (boards: LiveBoardState[]) => void
  updateLiveBoard: (board: LiveBoardState) => void
  removeLiveBoard: (mac: string) => void
  getLiveBoardAtPosition: (shelf: number, tray: 'front' | 'back', slot: number) => LiveBoardState | null

  // History actions
  addTelemetryEntry: (mac: string, entry: TelemetryEntry) => void
  addTestRun: (mac: string, run: TestRunSummary) => void
  clearBoardHistory: (mac: string) => void
  getTelemetryHistory: (mac: string) => TelemetryEntry[]
  getTestRuns: (mac: string) => TestRunSummary[]
}

export const useStore = create<AppState>((set, get) => ({
  // Initial state
  connected: false,
  currentInterface: null,
  interfaces: [],
  controlMode: 'fbc',
  sonomaBoards: [],
  selectedSonomaBoard: null,
  boards: [],
  selectedBoard: null,
  liveBoards: new Map(),
  rackConfig: {
    shelves: 11,
    boards_per_tray: 4,
    dual_tray: true,
    assignments: [],
  },
  telemetryHistory: new Map(),
  testRuns: new Map(),

  // Actions
  setConnected: (connected) => set({ connected }),
  setCurrentInterface: (currentInterface) => set({ currentInterface }),
  setInterfaces: (interfaces) => set({ interfaces }),
  setBoards: (boards) => set({ boards }),
  setSelectedBoard: (selectedBoard) => set({ selectedBoard }),
  setRackConfig: (rackConfig) => set({ rackConfig }),

  // Sonoma actions
  setControlMode: (controlMode) => set({ controlMode }),
  setSonomaBoards: (sonomaBoards) => set({ sonomaBoards }),
  setSelectedSonomaBoard: (selectedSonomaBoard) => set({ selectedSonomaBoard }),

  getBoardAtPosition: (shelf, tray, slot) => {
    const { boards, rackConfig } = get()
    const assignment = rackConfig.assignments.find(
      (a) =>
        a.position.shelf === shelf &&
        a.position.tray === tray &&
        a.position.slot === slot
    )
    if (!assignment) return null
    return boards.find((b) => b.mac === assignment.mac) || null
  },

  // Live board actions
  setLiveBoards: (boards) => {
    const newMap = new Map<string, LiveBoardState>()
    for (const board of boards) {
      newMap.set(board.mac, board)
    }
    set({ liveBoards: newMap })
  },

  updateLiveBoard: (board) => {
    const liveBoards = new Map(get().liveBoards)
    liveBoards.set(board.mac, board)
    set({ liveBoards })
  },

  removeLiveBoard: (mac) => {
    const liveBoards = new Map(get().liveBoards)
    const board = liveBoards.get(mac)
    if (board) {
      // Mark as offline instead of removing
      liveBoards.set(mac, { ...board, online: false })
    }
    set({ liveBoards })
  },

  getLiveBoardAtPosition: (shelf, tray, slot) => {
    const { liveBoards } = get()
    for (const board of liveBoards.values()) {
      if (board.position &&
          board.position.shelf === shelf &&
          board.position.tray === tray &&
          board.position.slot === slot) {
        return board
      }
    }
    return null
  },

  // History actions
  addTelemetryEntry: (mac, entry) => {
    const history = get().telemetryHistory
    const entries = history.get(mac) || []
    entries.push(entry)
    // Keep only last MAX_HISTORY_ENTRIES
    if (entries.length > MAX_HISTORY_ENTRIES) {
      entries.shift()
    }
    const newHistory = new Map(history)
    newHistory.set(mac, entries)
    set({ telemetryHistory: newHistory })
  },

  addTestRun: (mac, run) => {
    const runs = get().testRuns
    const boardRuns = runs.get(mac) || []
    boardRuns.push(run)
    // Keep only last 100 runs
    if (boardRuns.length > 100) {
      boardRuns.shift()
    }
    const newRuns = new Map(runs)
    newRuns.set(mac, boardRuns)
    set({ testRuns: newRuns })
  },

  clearBoardHistory: (mac) => {
    const history = new Map(get().telemetryHistory)
    const runs = new Map(get().testRuns)
    history.delete(mac)
    runs.delete(mac)
    set({ telemetryHistory: history, testRuns: runs })
  },

  getTelemetryHistory: (mac) => {
    return get().telemetryHistory.get(mac) || []
  },

  getTestRuns: (mac) => {
    return get().testRuns.get(mac) || []
  },
}))
