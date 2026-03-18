import { useState } from 'react'
import { useStore } from '../store'
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
  const { connected, selectedBoard, boards } = useStore()
  const [collapsed, setCollapsed] = useState(false)

  const selectedBoardInfo = boards.find(b => b.mac === selectedBoard)

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

      {/* Connection Status */}
      <div className="sidebar-status">
        <div className={`status-dot ${connected ? 'connected' : 'disconnected'}`} />
        {!collapsed && (
          <span className="status-text">
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        )}
      </div>

      {/* Selected Board */}
      {selectedBoard && !collapsed && (
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

      {/* Navigation */}
      <div className="sidebar-nav">
        {navItems.map(item => {
          const disabled = item.requiresBoard && !selectedBoard
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
          </div>
        </div>
      )}
    </nav>
  )
}
