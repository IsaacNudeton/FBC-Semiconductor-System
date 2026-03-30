import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore, SonomaBoardInfo } from '../store'
import './Sidebar.css'

export type ViewType =
  | 'overview'
  | 'board-detail'
  | 'analog'
  | 'power'
  | 'vectors'
  | 'config'
  | 'eeprom'
  | 'testplan'
  | 'facility'
  | 'firmware'
  | 'fleet-terminal'
  | 'pattern-converter'

interface SidebarProps {
  activeView: ViewType
  onViewChange: (view: ViewType) => void
}

export default function Sidebar({ activeView, onViewChange }: SidebarProps) {
  const {
    connected, selectedBoard, boards,
    controlMode, setControlMode,
    sonomaBoards, setSonomaBoards,
    selectedSonomaBoard, setSelectedSonomaBoard,
  } = useStore()
  const [collapsed, setCollapsed] = useState(false)
  const [scanRange, setScanRange] = useState('101-144')
  const [sonomaUser, setSonomaUser] = useState('root')
  const [sonomaPassword, setSonomaPassword] = useState('')
  const [scanning, setScanning] = useState(false)

  const selectedBoardInfo = boards.find(b => b.mac === selectedBoard)

  // Determine if a board is selected in the current mode
  const hasBoardSelected = controlMode === 'fbc' ? !!selectedBoard : !!selectedSonomaBoard

  const navItems: { id: ViewType; label: string; icon: string; requiresBoard?: boolean }[] = [
    { id: 'overview', label: 'Rack Overview', icon: '🏗️' },
    { id: 'board-detail', label: 'Board Details', icon: '📊', requiresBoard: true },
    { id: 'analog', label: 'Analog Monitor', icon: '📈', requiresBoard: true },
    { id: 'power', label: 'Power Control', icon: '⚡', requiresBoard: true },
    { id: 'vectors', label: 'Vector Engine', icon: '🔄', requiresBoard: true },
    { id: 'config', label: 'Device Config', icon: '⚙️', requiresBoard: true },
    { id: 'eeprom', label: 'EEPROM', icon: '💾', requiresBoard: true },
    { id: 'testplan', label: 'Test Plan', icon: '📋' },
    { id: 'facility', label: 'Facility', icon: '🏭' },
    { id: 'firmware', label: 'Firmware Update', icon: '🔧' },
    { id: 'fleet-terminal', label: 'Fleet Terminal', icon: '>' },
    { id: 'pattern-converter', label: 'Pattern Converter', icon: '~' },
  ]

  const handleScan = async () => {
    const parts = scanRange.split('-')
    if (parts.length !== 2) return

    const start = parseInt(parts[0])
    const end = parseInt(parts[1])
    if (isNaN(start) || isNaN(end)) return

    setScanning(true)
    try {
      const results = await invoke<SonomaBoardInfo[]>('sonoma_scan_range', {
        start, end, user: sonomaUser, password: sonomaPassword,
      })
      setSonomaBoards(results)
    } catch (e) {
      console.error('Scan failed:', e)
    } finally {
      setScanning(false)
    }
  }

  const handleSelectSonoma = (ip: string) => {
    setSelectedSonomaBoard(ip)
  }

  return (
    <nav className={`sidebar ${collapsed ? 'collapsed' : ''}`}>
      <div className="sidebar-header">
        <div className="logo">
          <span className="logo-icon">◈</span>
          {!collapsed && <span className="logo-text">FBC System</span>}
        </div>
        <button
          className="collapse-btn"
          onClick={() => setCollapsed(!collapsed)}
          title={collapsed ? 'Expand' : 'Collapse'}
        >
          {collapsed ? '→' : '←'}
        </button>
      </div>

      {/* Mode Toggle */}
      {!collapsed && (
        <div className="mode-toggle">
          <button
            className={`mode-btn ${controlMode === 'fbc' ? 'active' : ''}`}
            onClick={() => setControlMode('fbc')}
          >
            FBC
          </button>
          <button
            className={`mode-btn ${controlMode === 'sonoma' ? 'active' : ''}`}
            onClick={() => setControlMode('sonoma')}
          >
            Sonoma
          </button>
        </div>
      )}

      {/* Connection Status */}
      {controlMode === 'fbc' && (
        <div className="sidebar-status">
          <div className={`status-dot ${connected ? 'connected' : 'disconnected'}`} />
          {!collapsed && (
            <span className="status-text">
              {connected ? 'Connected' : 'Disconnected'}
            </span>
          )}
        </div>
      )}

      {/* Sonoma Controls */}
      {controlMode === 'sonoma' && !collapsed && (
        <div className="sonoma-section">
          <div className="sonoma-connect">
            <input
              type="text"
              value={scanRange}
              onChange={e => setScanRange(e.target.value)}
              placeholder="101-144"
              className="scan-input"
            />
            <div className="sonoma-creds">
              <input
                type="text"
                value={sonomaUser}
                onChange={e => setSonomaUser(e.target.value)}
                placeholder="user"
                className="cred-input"
              />
              <input
                type="password"
                value={sonomaPassword}
                onChange={e => setSonomaPassword(e.target.value)}
                placeholder="password"
                className="cred-input"
              />
            </div>
            <button
              className="btn-scan"
              onClick={handleScan}
              disabled={scanning}
            >
              {scanning ? 'Scanning...' : 'Scan'}
            </button>
          </div>

          {/* Sonoma Board List */}
          <div className="sonoma-boards">
            {sonomaBoards.filter(b => b.alive).map(board => (
              <button
                key={board.ip}
                className={`sonoma-board-item ${selectedSonomaBoard === board.ip ? 'selected' : ''}`}
                onClick={() => handleSelectSonoma(board.ip)}
              >
                <span className="status-dot connected" />
                <span className="board-ip">{board.ip}</span>
                {board.hostname && (
                  <span className="board-hostname">{board.hostname}</span>
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Selected Board (FBC mode) */}
      {controlMode === 'fbc' && selectedBoard && !collapsed && (
        <div className="selected-board">
          <div className="selected-label">Selected Board</div>
          <div className="selected-mac">{selectedBoard}</div>
          {selectedBoardInfo && (
            <div className={`selected-state state-${selectedBoardInfo.state}`}>
              {selectedBoardInfo.state}
            </div>
          )}
        </div>
      )}

      {/* Selected Board (Sonoma mode) */}
      {controlMode === 'sonoma' && selectedSonomaBoard && !collapsed && (
        <div className="selected-board">
          <div className="selected-label">Selected Board</div>
          <div className="selected-mac">{selectedSonomaBoard}</div>
          <div className="selected-state state-idle">Sonoma SSH</div>
        </div>
      )}

      {/* Navigation */}
      <div className="sidebar-nav">
        {navItems.map(item => {
          const disabled = item.requiresBoard && !hasBoardSelected
          return (
            <button
              key={item.id}
              className={`nav-item ${activeView === item.id ? 'active' : ''} ${disabled ? 'disabled' : ''}`}
              onClick={() => !disabled && onViewChange(item.id)}
              title={collapsed ? item.label : undefined}
              disabled={disabled}
            >
              <span className="nav-icon">{item.icon}</span>
              {!collapsed && <span className="nav-label">{item.label}</span>}
            </button>
          )
        })}
      </div>

      {/* Board Count */}
      {!collapsed && (
        <div className="sidebar-footer">
          <div className="board-summary">
            {controlMode === 'fbc' ? (
              <>
                <div className="summary-row">
                  <span>Total Boards:</span>
                  <span className="value">{boards.length}</span>
                </div>
                <div className="summary-row">
                  <span>Running:</span>
                  <span className="value running">{boards.filter(b => b.state === 'running').length}</span>
                </div>
                <div className="summary-row">
                  <span>Errors:</span>
                  <span className="value error">{boards.filter(b => b.state === 'error').length}</span>
                </div>
              </>
            ) : (
              <>
                <div className="summary-row">
                  <span>Boards Found:</span>
                  <span className="value">{sonomaBoards.filter(b => b.alive).length}</span>
                </div>
                <div className="summary-row">
                  <span>Scanned:</span>
                  <span className="value">{sonomaBoards.length}</span>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </nav>
  )
}
