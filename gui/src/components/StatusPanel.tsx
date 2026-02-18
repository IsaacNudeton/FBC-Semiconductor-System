import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './StatusPanel.css'

interface BoardStatus {
  state: string
  cycles: number
  errors: number
  temp_c: number
  fpga_vccint_mv: number
  fpga_vccaux_mv: number
}

export default function StatusPanel() {
  const { boards, selectedBoard, setSelectedBoard, connected } = useStore()
  const [status, setStatus] = useState<BoardStatus | null>(null)
  const [loading, setLoading] = useState(false)

  const selectedBoardInfo = boards.find((b) => b.mac === selectedBoard)

  // Fetch status when board is selected
  useEffect(() => {
    if (!selectedBoard || !connected) {
      setStatus(null)
      return
    }

    const fetchStatus = async () => {
      setLoading(true)
      try {
        const s = await invoke<BoardStatus>('get_board_status', { mac: selectedBoard })
        setStatus(s)
      } catch (e) {
        console.error('Failed to get status:', e)
        setStatus(null)
      }
      setLoading(false)
    }

    fetchStatus()
    const interval = setInterval(fetchStatus, 2000)
    return () => clearInterval(interval)
  }, [selectedBoard, connected])

  const handleStart = async () => {
    if (!selectedBoard) return
    try {
      await invoke('start_board', { mac: selectedBoard })
    } catch (e) {
      console.error('Start failed:', e)
    }
  }

  const handleStop = async () => {
    if (!selectedBoard) return
    try {
      await invoke('stop_board', { mac: selectedBoard })
    } catch (e) {
      console.error('Stop failed:', e)
    }
  }

  const handleReset = async () => {
    if (!selectedBoard) return
    try {
      await invoke('reset_board', { mac: selectedBoard })
    } catch (e) {
      console.error('Reset failed:', e)
    }
  }

  return (
    <div className="status-panel">
      <h2>Board Status</h2>

      {!connected && (
        <div className="status-message">Not connected. Select interface and connect.</div>
      )}

      {connected && !selectedBoard && (
        <div className="status-message">Click a board in the rack to view details.</div>
      )}

      {connected && selectedBoard && selectedBoardInfo && (
        <>
          <div className="board-info">
            <div className="info-row">
              <span className="label">MAC:</span>
              <span className="value mono">{selectedBoardInfo.mac}</span>
            </div>
            <div className="info-row">
              <span className="label">Serial:</span>
              <span className="value">{selectedBoardInfo.serial}</span>
            </div>
            <div className="info-row">
              <span className="label">Firmware:</span>
              <span className="value">v{selectedBoardInfo.fw_version}</span>
            </div>
            <div className="info-row">
              <span className="label">BIM:</span>
              <span className="value">
                {selectedBoardInfo.has_bim ? `Type ${selectedBoardInfo.bim_type}` : 'None'}
              </span>
            </div>
          </div>

          <div className="status-section">
            <h3>Live Status</h3>
            {loading && !status && <div className="loading">Loading...</div>}
            {status && (
              <>
                <div className="info-row">
                  <span className="label">State:</span>
                  <span className={`value state-${status.state}`}>{status.state}</span>
                </div>
                <div className="info-row">
                  <span className="label">Cycles:</span>
                  <span className="value">{status.cycles.toLocaleString()}</span>
                </div>
                <div className="info-row">
                  <span className="label">Errors:</span>
                  <span className={`value ${status.errors > 0 ? 'error' : ''}`}>
                    {status.errors.toLocaleString()}
                  </span>
                </div>
                <div className="info-row">
                  <span className="label">Temperature:</span>
                  <span className="value">{status.temp_c.toFixed(1)}°C</span>
                </div>
                <div className="info-row">
                  <span className="label">VCCINT:</span>
                  <span className="value">{status.fpga_vccint_mv} mV</span>
                </div>
                <div className="info-row">
                  <span className="label">VCCAUX:</span>
                  <span className="value">{status.fpga_vccaux_mv} mV</span>
                </div>
              </>
            )}
          </div>

          <div className="controls">
            <button className="btn-start" onClick={handleStart}>
              Start
            </button>
            <button className="btn-stop" onClick={handleStop}>
              Stop
            </button>
            <button className="btn-reset" onClick={handleReset}>
              Reset
            </button>
          </div>

          <button className="btn-close" onClick={() => setSelectedBoard(null)}>
            Close
          </button>
        </>
      )}

      {/* Board list */}
      <div className="board-list">
        <h3>All Boards ({boards.length})</h3>
        <div className="board-list-items">
          {boards.map((board) => (
            <div
              key={board.mac}
              className={`board-item ${board.mac === selectedBoard ? 'selected' : ''} state-${board.state}`}
              onClick={() => setSelectedBoard(board.mac)}
            >
              <span className="board-mac">{board.mac}</span>
              <span className={`board-state state-${board.state}`}>{board.state}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
