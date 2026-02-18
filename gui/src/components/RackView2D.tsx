import { useState } from 'react'
import { useStore } from '../store'
import { useRealtimeBoards, formatRunTime } from '../hooks/useRealtimeBoards'
import './RackView2D.css'

interface BoardCardProps {
  shelf: number
  tray: 'front' | 'back'
  position: 'A' | 'B'
  onClick: () => void
}

function BoardCard({ shelf, tray, position, onClick }: BoardCardProps) {
  const { getLiveBoardAtPosition, selectedBoard, setSelectedBoard } = useStore()

  // Map position A/B to slot 1/2
  const slot = position === 'A' ? 1 : 2

  // Find live board at this position (auto-detected from switch)
  const board = getLiveBoardAtPosition(shelf, tray, slot)
  const isSelected = board && selectedBoard === board.mac

  const handleClick = () => {
    if (board) {
      setSelectedBoard(board.mac)
    }
    onClick()
  }

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    // TODO: Show context menu
  }

  // Determine state class (handle offline boards)
  const getStateClass = () => {
    if (!board) return 'empty'
    if (!board.online) return 'offline'
    return board.state || 'unknown'
  }

  const stateClass = getStateClass()
  const isRunning = board?.state === 'running' && board?.online

  // Get status text
  const getStatusText = () => {
    if (!board) return ''
    if (!board.online) return 'Offline'
    switch (board.state) {
      case 'running': return 'Running'
      case 'idle': return 'Idle'
      case 'done': return 'Complete'
      case 'error': return 'Error'
      default: return 'Unknown'
    }
  }

  return (
    <div
      className={`board-card ${stateClass} ${isSelected ? 'selected' : ''}`}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
    >
      {board ? (
        <>
          {/* Header row */}
          <div className="card-header">
            <div className="position-info">
              <span className="pos-label">Pos {position}</span>
              <span className="board-ip">{board.mac.slice(-8)}</span>
            </div>
            <div className="status-badge">
              <span className={`status-dot ${stateClass} ${isRunning ? 'pulsing' : ''}`} />
              <span className={`status-text ${stateClass}`}>
                {getStatusText()}
              </span>
            </div>
          </div>

          {/* Metrics row - now with real data */}
          <div className="metrics-row">
            <div className="metric">
              <span className="metric-icon">T</span>
              <span className="metric-value">{board.temp_c.toFixed(1)}C</span>
            </div>
            <div className="metric">
              <span className="metric-icon">C</span>
              <span className="metric-value">{board.cycles.toLocaleString()}</span>
            </div>
            {board.errors > 0 && (
              <div className="metric error">
                <span className="metric-icon">E</span>
                <span className="metric-value">{board.errors}</span>
              </div>
            )}
            {isRunning && (
              <span className="run-time">{formatRunTime(board.run_time_ms)}</span>
            )}
          </div>

          {/* Progress indicator for running boards */}
          {isRunning && (
            <div className="progress-container">
              <div className="progress-bar progress-indeterminate" />
            </div>
          )}
        </>
      ) : (
        <div className="empty-slot">
          <span className="pos-label">Pos {position}</span>
          <span className="no-hardware">No Hardware</span>
        </div>
      )}
    </div>
  )
}

interface ShelfCardProps {
  shelfNum: number
  onBoardClick: (shelf: number, tray: 'front' | 'back', position: 'A' | 'B') => void
}

function ShelfCard({ shelfNum, onBoardClick }: ShelfCardProps) {
  return (
    <div className="shelf-card">
      {/* Shelf Header */}
      <div className="shelf-header">
        <div className="shelf-icon">🗄</div>
        <div className="shelf-info">
          <span className="shelf-name">Shelf {String(shelfNum).padStart(2, '0')}</span>
          <span className="shelf-subtitle">Rear & Front Trays</span>
        </div>
      </div>

      {/* Trays Container */}
      <div className="trays-container">
        {/* Rear Tray (on top - operator's perspective) */}
        <div className="tray-section">
          <div className="tray-label">
            <span className="tray-dot rear" />
            <span>Rear Tray</span>
          </div>
          <div className="tray-positions">
            <BoardCard
              shelf={shelfNum}
              tray="back"
              position="B"
              onClick={() => onBoardClick(shelfNum, 'back', 'B')}
            />
            <BoardCard
              shelf={shelfNum}
              tray="back"
              position="A"
              onClick={() => onBoardClick(shelfNum, 'back', 'A')}
            />
          </div>
        </div>

        {/* Front Tray (on bottom) */}
        <div className="tray-section">
          <div className="tray-label">
            <span className="tray-dot front" />
            <span>Front Tray</span>
          </div>
          <div className="tray-positions">
            <BoardCard
              shelf={shelfNum}
              tray="front"
              position="B"
              onClick={() => onBoardClick(shelfNum, 'front', 'B')}
            />
            <BoardCard
              shelf={shelfNum}
              tray="front"
              position="A"
              onClick={() => onBoardClick(shelfNum, 'front', 'A')}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

interface RackView2DProps {
  onBoardSelect: (mac: string) => void
  onSlotSelect?: (shelf: number, tray: 'front' | 'back', position: number) => void
}

export default function RackView2D({ onBoardSelect, onSlotSelect }: RackView2DProps) {
  const { liveBoards, connected } = useStore()
  const [statusFilter, setStatusFilter] = useState('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [showMetrics, setShowMetrics] = useState(true)

  // Enable realtime monitoring
  useRealtimeBoards(connected)

  // Convert live boards map to array for stats
  const liveBoardsArray = Array.from(liveBoards.values())

  // Stats from live boards
  const totalSlots = 44
  const onlineBoards = liveBoardsArray.filter(b => b.online)
  const occupiedSlots = onlineBoards.length
  const runningCount = onlineBoards.filter(b => b.state === 'running').length
  const errorCount = onlineBoards.filter(b => b.state === 'error').length
  const idleCount = onlineBoards.filter(b => b.state === 'idle').length

  const handleBoardClick = (shelf: number, tray: 'front' | 'back', position: 'A' | 'B') => {
    const slot = position === 'A' ? 1 : 2
    // Find board at this position and call onBoardSelect if found
    const board = liveBoardsArray.find(b =>
      b.position &&
      b.position.shelf === shelf &&
      b.position.tray === tray &&
      b.position.slot === slot
    )
    if (board) {
      onBoardSelect(board.mac)
    }
    onSlotSelect?.(shelf, tray, slot)
  }

  // Shelves from 11 (top) to 1 (bottom)
  const shelves = Array.from({ length: 11 }, (_, i) => 11 - i)

  return (
    <div className="rack-view-2d">
      {/* Header */}
      <div className="view-header">
        <div className="header-title">
          <h1>Chamber Overview</h1>
          <span className="header-subtitle">11 Shelves × 4 Boards • Real-time Monitoring</span>
        </div>
        <div className="header-controls">
          <button
            className={`control-btn ${showMetrics ? 'active' : ''}`}
            onClick={() => setShowMetrics(!showMetrics)}
          >
            Show Metrics
          </button>
          <button className="control-btn">Legend</button>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="filter-bar">
        <span className="filter-label">Filter:</span>
        <select
          className="filter-select"
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value)}
        >
          <option value="all">All Status</option>
          <option value="running">Running</option>
          <option value="idle">Idle</option>
          <option value="error">Error</option>
          <option value="done">Complete</option>
        </select>
        <input
          type="text"
          className="search-input"
          placeholder="🔍 Search board ID..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
        <button className="clear-btn" onClick={() => { setStatusFilter('all'); setSearchQuery(''); }}>
          Clear
        </button>
      </div>

      {/* Batch Operations */}
      <div className="batch-toolbar">
        <span className="toolbar-label">Batch Operations:</span>
        <button className="action-btn primary">▶ Start All</button>
        <button className="action-btn">⏹ Stop All</button>
        <button className="action-btn">🔄 Restart All</button>
        <div className="toolbar-divider" />
        <button className="action-btn">📂 LOT Manager</button>
        <button className="action-btn">🔧 Device Manager</button>
      </div>

      {/* Stats Summary */}
      <div className="stats-bar">
        <div className="stat-item">
          <span className="stat-value">{occupiedSlots}/{totalSlots}</span>
          <span className="stat-label">Slots</span>
        </div>
        <div className="stat-item running">
          <span className="stat-value">{runningCount}</span>
          <span className="stat-label">Running</span>
        </div>
        <div className="stat-item idle">
          <span className="stat-value">{idleCount}</span>
          <span className="stat-label">Idle</span>
        </div>
        <div className="stat-item error">
          <span className="stat-value">{errorCount}</span>
          <span className="stat-label">Errors</span>
        </div>
      </div>

      {/* Shelves List */}
      <div className="shelves-list">
        {shelves.map(shelf => (
          <ShelfCard
            key={shelf}
            shelfNum={shelf}
            onBoardClick={handleBoardClick}
          />
        ))}
      </div>
    </div>
  )
}
