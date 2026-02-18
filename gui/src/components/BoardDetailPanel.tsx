import { useState, useEffect, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './BoardDetailPanel.css'

interface DetailedBoardStatus {
  // Basic
  state: string
  cycles: number
  errors: number
  temp_c: number

  // FPGA
  fpga_vccint_mv: number
  fpga_vccaux_mv: number
  fpga_vccbram_mv: number
  fpga_temp_c: number

  // Runtime
  uptime_secs: number
  vectors_loaded: boolean
  vector_count: number
  current_vector: number

  // Timing
  freq_sel: number
  vec_clock_hz: number
}

interface EepromInfo {
  magic: number
  version: number
  bim_type: number
  hw_revision: number
  serial_number: number
  manufacture_date: number
  vendor: string
  part_number: string
  description: string
  is_programmed: boolean
  is_valid: boolean
}

export default function BoardDetailPanel() {
  const { selectedBoard, boards, connected, addTelemetryEntry, getTelemetryHistory, getTestRuns } = useStore()
  const [status, setStatus] = useState<DetailedBoardStatus | null>(null)
  const [eeprom, setEeprom] = useState<EepromInfo | null>(null)
  const [loading, setLoading] = useState(false)
  const [activeSection, setActiveSection] = useState<'status' | 'hardware' | 'vectors' | 'history'>('status')

  const board = boards.find(b => b.mac === selectedBoard)

  // Get history for selected board
  const telemetryHistory = useMemo(() => {
    return selectedBoard ? getTelemetryHistory(selectedBoard) : []
  }, [selectedBoard, getTelemetryHistory])

  const testRuns = useMemo(() => {
    return selectedBoard ? getTestRuns(selectedBoard) : []
  }, [selectedBoard, getTestRuns])

  useEffect(() => {
    if (!selectedBoard || !connected) {
      setStatus(null)
      setEeprom(null)
      return
    }

    const fetchData = async () => {
      setLoading(true)
      try {
        const [statusResult, eepromResult] = await Promise.all([
          invoke<DetailedBoardStatus>('get_detailed_status', { mac: selectedBoard }).catch(() => null),
          invoke<EepromInfo>('get_eeprom_info', { mac: selectedBoard }).catch(() => null),
        ])
        setStatus(statusResult)
        setEeprom(eepromResult)

        // Store telemetry entry for history
        if (statusResult && selectedBoard) {
          addTelemetryEntry(selectedBoard, {
            timestamp: Date.now(),
            cycles: statusResult.cycles,
            errors: statusResult.errors,
            temp_c: statusResult.temp_c,
            state: statusResult.state,
            fpga_vccint_mv: statusResult.fpga_vccint_mv,
            fpga_vccaux_mv: statusResult.fpga_vccaux_mv,
          })
        }
      } catch (e) {
        console.error('Failed to fetch board details:', e)
      }
      setLoading(false)
    }

    fetchData()
    const interval = setInterval(fetchData, 2000)
    return () => clearInterval(interval)
  }, [selectedBoard, connected])

  if (!selectedBoard || !board) {
    return (
      <div className="board-detail-panel">
        <div className="no-board-message">
          <span className="icon">📋</span>
          <h3>No Board Selected</h3>
          <p>Click on a board in the rack view or select from the board list to view detailed information.</p>
        </div>
      </div>
    )
  }

  const formatUptime = (secs: number): string => {
    const hours = Math.floor(secs / 3600)
    const mins = Math.floor((secs % 3600) / 60)
    const s = secs % 60
    return `${hours}h ${mins}m ${s}s`
  }

  const formatFrequency = (sel: number): string => {
    const freqs = ['5 MHz', '10 MHz', '25 MHz', '50 MHz', '100 MHz']
    return freqs[sel] || 'Unknown'
  }

  const formatDate = (timestamp: number): string => {
    if (timestamp === 0 || timestamp === 0xFFFFFFFF) return 'Not set'
    return new Date(timestamp * 1000).toLocaleDateString()
  }

  return (
    <div className="board-detail-panel">
      {/* Header */}
      <div className="detail-header">
        <div className="board-identity">
          <h2>{eeprom?.part_number || 'FBC Board'}</h2>
          <span className="board-mac">{selectedBoard}</span>
        </div>
        <div className={`board-state-badge state-${board.state}`}>
          {board.state.toUpperCase()}
        </div>
      </div>

      {/* Section Tabs */}
      <div className="section-tabs">
        <button
          className={activeSection === 'status' ? 'active' : ''}
          onClick={() => setActiveSection('status')}
        >
          Live Status
        </button>
        <button
          className={activeSection === 'hardware' ? 'active' : ''}
          onClick={() => setActiveSection('hardware')}
        >
          Hardware
        </button>
        <button
          className={activeSection === 'vectors' ? 'active' : ''}
          onClick={() => setActiveSection('vectors')}
        >
          Vectors
        </button>
        <button
          className={activeSection === 'history' ? 'active' : ''}
          onClick={() => setActiveSection('history')}
        >
          History
        </button>
      </div>

      {/* Content */}
      <div className="detail-content">
        {loading && !status && <div className="loading-spinner" />}

        {activeSection === 'status' && (
          <div className="section-status">
            {/* Quick Stats */}
            <div className="stats-grid">
              <div className="stat-card">
                <div className="stat-value">{status?.cycles.toLocaleString() || '—'}</div>
                <div className="stat-label">Total Cycles</div>
              </div>
              <div className="stat-card">
                <div className={`stat-value ${(status?.errors || 0) > 0 ? 'error' : ''}`}>
                  {status?.errors.toLocaleString() || '0'}
                </div>
                <div className="stat-label">Errors</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{status?.temp_c?.toFixed(1) || '—'}°C</div>
                <div className="stat-label">Die Temperature</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{status ? formatUptime(status.uptime_secs) : '—'}</div>
                <div className="stat-label">Uptime</div>
              </div>
            </div>

            {/* FPGA Voltages */}
            <div className="info-section">
              <h4>FPGA Power Rails</h4>
              <div className="info-grid">
                <div className="info-item">
                  <span className="info-label">VCCINT</span>
                  <span className="info-value">{status?.fpga_vccint_mv || '—'} mV</span>
                  <div className="voltage-bar">
                    <div
                      className="voltage-fill"
                      style={{ width: `${((status?.fpga_vccint_mv || 0) / 1100) * 100}%` }}
                    />
                  </div>
                </div>
                <div className="info-item">
                  <span className="info-label">VCCAUX</span>
                  <span className="info-value">{status?.fpga_vccaux_mv || '—'} mV</span>
                  <div className="voltage-bar">
                    <div
                      className="voltage-fill"
                      style={{ width: `${((status?.fpga_vccaux_mv || 0) / 1900) * 100}%` }}
                    />
                  </div>
                </div>
                <div className="info-item">
                  <span className="info-label">VCCBRAM</span>
                  <span className="info-value">{status?.fpga_vccbram_mv || '—'} mV</span>
                  <div className="voltage-bar">
                    <div
                      className="voltage-fill"
                      style={{ width: `${((status?.fpga_vccbram_mv || 0) / 1100) * 100}%` }}
                    />
                  </div>
                </div>
                <div className="info-item">
                  <span className="info-label">FPGA Temp</span>
                  <span className="info-value">{status?.fpga_temp_c?.toFixed(1) || '—'}°C</span>
                  <div className="temp-bar">
                    <div
                      className="temp-fill"
                      style={{ width: `${((status?.fpga_temp_c || 0) / 100) * 100}%` }}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* Controls */}
            <div className="control-section">
              <h4>Test Control</h4>
              <div className="control-buttons">
                <button className="btn btn-start" onClick={() => invoke('start_board', { mac: selectedBoard })}>
                  ▶ Start
                </button>
                <button className="btn btn-stop" onClick={() => invoke('stop_board', { mac: selectedBoard })}>
                  ◼ Stop
                </button>
                <button className="btn btn-reset" onClick={() => invoke('reset_board', { mac: selectedBoard })}>
                  ↺ Reset
                </button>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'hardware' && (
          <div className="section-hardware">
            {/* Board Identity */}
            <div className="info-section">
              <h4>Board Identity</h4>
              <div className="info-list">
                <div className="info-row">
                  <span className="label">MAC Address</span>
                  <span className="value mono">{selectedBoard}</span>
                </div>
                <div className="info-row">
                  <span className="label">Serial Number</span>
                  <span className="value mono">{board.serial || eeprom?.serial_number || '—'}</span>
                </div>
                <div className="info-row">
                  <span className="label">Hardware Rev</span>
                  <span className="value">{board.hw_revision || eeprom?.hw_revision || '—'}</span>
                </div>
                <div className="info-row">
                  <span className="label">Firmware</span>
                  <span className="value">v{board.fw_version || '—'}</span>
                </div>
              </div>
            </div>

            {/* EEPROM / BIM Info */}
            <div className="info-section">
              <h4>BIM Configuration</h4>
              {eeprom?.is_programmed ? (
                <div className="info-list">
                  <div className="info-row">
                    <span className="label">BIM Type</span>
                    <span className="value">{board.bim_type || eeprom.bim_type}</span>
                  </div>
                  <div className="info-row">
                    <span className="label">Vendor</span>
                    <span className="value">{eeprom.vendor || '—'}</span>
                  </div>
                  <div className="info-row">
                    <span className="label">Part Number</span>
                    <span className="value">{eeprom.part_number || '—'}</span>
                  </div>
                  <div className="info-row">
                    <span className="label">Description</span>
                    <span className="value">{eeprom.description || '—'}</span>
                  </div>
                  <div className="info-row">
                    <span className="label">Manufacture Date</span>
                    <span className="value">{formatDate(eeprom.manufacture_date)}</span>
                  </div>
                  <div className="info-row">
                    <span className="label">EEPROM Valid</span>
                    <span className={`value ${eeprom.is_valid ? 'success' : 'error'}`}>
                      {eeprom.is_valid ? '✓ Valid' : '✗ Invalid CRC'}
                    </span>
                  </div>
                </div>
              ) : (
                <div className="eeprom-not-programmed">
                  <span className="icon">⚠️</span>
                  <p>EEPROM not programmed</p>
                  <button className="btn btn-secondary">Program EEPROM</button>
                </div>
              )}
            </div>
          </div>
        )}

        {activeSection === 'vectors' && (
          <div className="section-vectors">
            <div className="info-section">
              <h4>Vector Engine Status</h4>
              <div className="info-list">
                <div className="info-row">
                  <span className="label">Vectors Loaded</span>
                  <span className={`value ${status?.vectors_loaded ? 'success' : ''}`}>
                    {status?.vectors_loaded ? '✓ Yes' : '✗ No'}
                  </span>
                </div>
                <div className="info-row">
                  <span className="label">Vector Count</span>
                  <span className="value">{status?.vector_count?.toLocaleString() || '0'}</span>
                </div>
                <div className="info-row">
                  <span className="label">Current Vector</span>
                  <span className="value">{status?.current_vector?.toLocaleString() || '0'}</span>
                </div>
                <div className="info-row">
                  <span className="label">Clock Frequency</span>
                  <span className="value">{status ? formatFrequency(status.freq_sel) : '—'}</span>
                </div>
              </div>
            </div>

            {/* Progress */}
            {status?.vectors_loaded && (
              <div className="vector-progress">
                <div className="progress-header">
                  <span>Execution Progress</span>
                  <span>{((status.current_vector / status.vector_count) * 100).toFixed(1)}%</span>
                </div>
                <div className="progress-bar">
                  <div
                    className="progress-fill"
                    style={{ width: `${(status.current_vector / status.vector_count) * 100}%` }}
                  />
                </div>
              </div>
            )}

            {/* Vector Actions */}
            <div className="vector-actions">
              <button className="btn btn-primary">
                📁 Load Vectors
              </button>
              <button className="btn btn-secondary">
                ⚙️ Configure Pins
              </button>
            </div>
          </div>
        )}

        {activeSection === 'history' && (
          <div className="section-history">
            {/* Session Statistics */}
            <div className="info-section">
              <h4>Session Statistics</h4>
              <div className="stats-grid small">
                <div className="stat-card">
                  <div className="stat-value">{telemetryHistory.length}</div>
                  <div className="stat-label">Data Points</div>
                </div>
                <div className="stat-card">
                  <div className="stat-value">
                    {telemetryHistory.length > 0
                      ? Math.max(...telemetryHistory.map(e => e.temp_c)).toFixed(1)
                      : '—'}°C
                  </div>
                  <div className="stat-label">Peak Temp</div>
                </div>
                <div className="stat-card">
                  <div className="stat-value">
                    {telemetryHistory.length > 0
                      ? telemetryHistory[telemetryHistory.length - 1].cycles.toLocaleString()
                      : '0'}
                  </div>
                  <div className="stat-label">Latest Cycles</div>
                </div>
                <div className="stat-card">
                  <div className="stat-value">
                    {telemetryHistory.filter(e => e.errors > 0).length}
                  </div>
                  <div className="stat-label">Error Events</div>
                </div>
              </div>
            </div>

            {/* Recent Telemetry */}
            <div className="info-section">
              <h4>Recent Telemetry</h4>
              {telemetryHistory.length === 0 ? (
                <div className="empty-history">
                  <p>No telemetry data recorded yet.</p>
                  <p className="hint">Data is recorded when the board is connected and polled.</p>
                </div>
              ) : (
                <div className="telemetry-table">
                  <div className="table-header">
                    <span>Time</span>
                    <span>State</span>
                    <span>Cycles</span>
                    <span>Errors</span>
                    <span>Temp</span>
                  </div>
                  <div className="table-body">
                    {telemetryHistory.slice(-20).reverse().map((entry, idx) => (
                      <div key={idx} className={`table-row ${entry.errors > 0 ? 'has-error' : ''}`}>
                        <span className="mono">
                          {new Date(entry.timestamp).toLocaleTimeString()}
                        </span>
                        <span className={`state-badge state-${entry.state}`}>
                          {entry.state}
                        </span>
                        <span>{entry.cycles.toLocaleString()}</span>
                        <span className={entry.errors > 0 ? 'error' : ''}>
                          {entry.errors}
                        </span>
                        <span>{entry.temp_c.toFixed(1)}°C</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Temperature Trend (simple text-based) */}
            {telemetryHistory.length > 1 && (
              <div className="info-section">
                <h4>Temperature Trend</h4>
                <div className="temp-trend">
                  {telemetryHistory.slice(-30).map((entry, idx) => {
                    const minTemp = Math.min(...telemetryHistory.slice(-30).map(e => e.temp_c))
                    const maxTemp = Math.max(...telemetryHistory.slice(-30).map(e => e.temp_c))
                    const range = maxTemp - minTemp || 1
                    const height = ((entry.temp_c - minTemp) / range) * 100
                    return (
                      <div
                        key={idx}
                        className="trend-bar"
                        style={{ height: `${Math.max(10, height)}%` }}
                        title={`${entry.temp_c.toFixed(1)}°C`}
                      />
                    )
                  })}
                </div>
                <div className="trend-labels">
                  <span>{Math.min(...telemetryHistory.slice(-30).map(e => e.temp_c)).toFixed(1)}°C</span>
                  <span>{Math.max(...telemetryHistory.slice(-30).map(e => e.temp_c)).toFixed(1)}°C</span>
                </div>
              </div>
            )}

            {/* Actions */}
            <div className="history-actions">
              <button
                className="btn btn-secondary"
                onClick={() => {
                  // Export history as JSON
                  const data = {
                    mac: selectedBoard,
                    exported_at: new Date().toISOString(),
                    telemetry: telemetryHistory,
                    test_runs: testRuns,
                  }
                  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
                  const url = URL.createObjectURL(blob)
                  const a = document.createElement('a')
                  a.href = url
                  a.download = `board_${selectedBoard?.replace(/:/g, '-')}_history.json`
                  document.body.appendChild(a)
                  a.click()
                  document.body.removeChild(a)
                  URL.revokeObjectURL(url)
                }}
              >
                Export History
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
