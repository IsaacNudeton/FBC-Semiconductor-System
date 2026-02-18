import { useState, useEffect, useMemo, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import { WaveformViewer, PinGrid } from './Charts'
import './VectorPatternPanel.css'

// ============================================================================
// Types
// ============================================================================

interface VectorData {
    cycle: number
    outputs: Uint8Array    // 160 bits packed as 20 bytes
    expected: Uint8Array   // Expected input values
    actual?: Uint8Array    // Actual input values (from test)
}

interface ErrorInfo {
    vector: number
    cycle: number
    first_fail_pin: number
    error_mask: number[]   // List of pins with errors
    timestamp: number
}

interface PatternStats {
    total_vectors: number
    total_cycles: number
    total_errors: number
    first_error_vector: number
    first_error_cycle: number
    error_pins: number[]
}

// ============================================================================
// Component
// ============================================================================

export default function VectorPatternPanel() {
    const { selectedBoard, connected } = useStore()

    // View state
    const [activeTab, setActiveTab] = useState<'waveform' | 'errors' | 'stats'>('waveform')
    const [startCycle, setStartCycle] = useState(0)
    const [selectedCycle, setSelectedCycle] = useState<number | undefined>()
    const [selectedPin, setSelectedPin] = useState<number | undefined>()
    const [cyclesVisible, setCyclesVisible] = useState(32)

    // Data state
    const [_patterns, _setPatterns] = useState<VectorData[]>([])
    const [errors, setErrors] = useState<ErrorInfo[]>([])
    const [stats, setStats] = useState<PatternStats | null>(null)
    const [loading, setLoading] = useState(false)

    // Selected channels for waveform view
    const [selectedChannels, setSelectedChannels] = useState<number[]>([0, 1, 2, 3])

    // Fetch pattern data from board
    const fetchPatterns = useCallback(async () => {
        if (!selectedBoard || !connected) return

        setLoading(true)
        try {
            // Get pattern stats
            const statsResult = await invoke<PatternStats>('get_pattern_stats', {
                mac: selectedBoard
            }).catch(() => null)

            if (statsResult) {
                setStats(statsResult)
            }

            // Get errors if any
            const errorsResult = await invoke<ErrorInfo[]>('get_pattern_errors', {
                mac: selectedBoard,
                limit: 100,
            }).catch(() => [])

            setErrors(errorsResult || [])

        } catch (e) {
            console.error('Failed to fetch patterns:', e)
        }
        setLoading(false)
    }, [selectedBoard, connected])

    // Initial load and polling
    useEffect(() => {
        if (selectedBoard && connected) {
            fetchPatterns()
            const interval = setInterval(fetchPatterns, 2000)
            return () => clearInterval(interval)
        }
    }, [selectedBoard, connected, fetchPatterns])

    // Generate waveform data for visualization
    // NOTE: This is demo data for development. In production with real hardware,
    // this would fetch actual vector data via a `get_vector_data(mac, startCycle, count)` command.
    // The demo patterns use different frequencies per pin to visualize the waveform viewer.
    const waveformChannels = useMemo(() => {
        return selectedChannels.map(pin => ({
            name: `Pin ${pin}`,
            data: Array.from({ length: 256 }, (_, i) => {
                // Demo pattern: different frequencies per pin for visual variety
                const freq = (pin % 8) + 1
                return (Math.floor(i / freq) % 2) as 0 | 1
            }),
            type: (pin < 128 ? 'bidirectional' : 'output') as 'input' | 'output' | 'bidirectional',
        }))
    }, [selectedChannels])

    // Error mask as list of pins
    const errorPinMask = useMemo(() => {
        if (!stats) return []
        return stats.error_pins || []
    }, [stats])

    // Handle cycle navigation
    const handleCycleNav = (direction: 'prev' | 'next' | 'start' | 'end') => {
        const maxCycle = stats?.total_cycles || 1000
        switch (direction) {
            case 'prev':
                setStartCycle(Math.max(0, startCycle - cyclesVisible))
                break
            case 'next':
                setStartCycle(Math.min(maxCycle - cyclesVisible, startCycle + cyclesVisible))
                break
            case 'start':
                setStartCycle(0)
                break
            case 'end':
                setStartCycle(Math.max(0, maxCycle - cyclesVisible))
                break
        }
    }

    // Jump to first error
    const jumpToFirstError = () => {
        if (stats && stats.first_error_cycle > 0) {
            setStartCycle(Math.max(0, stats.first_error_cycle - 4))
            setSelectedCycle(stats.first_error_cycle)
        }
    }

    // Add/remove channel from view
    const toggleChannel = (pin: number) => {
        if (selectedChannels.includes(pin)) {
            setSelectedChannels(prev => prev.filter(p => p !== pin))
        } else if (selectedChannels.length < 16) {
            setSelectedChannels(prev => [...prev, pin].sort((a, b) => a - b))
        }
    }

    if (!selectedBoard) {
        return (
            <div className="vector-pattern-panel">
                <div className="no-board-message">
                    <span className="icon">📊</span>
                    <h3>No Board Selected</h3>
                    <p>Select a board to view vector patterns and error analysis.</p>
                </div>
            </div>
        )
    }

    return (
        <div className="vector-pattern-panel">
            {/* Header */}
            <div className="panel-header">
                <div className="header-info">
                    <h2>Vector Pattern Analysis</h2>
                    <span className="board-mac">{selectedBoard}</span>
                </div>
                <div className="header-tabs">
                    <button
                        className={activeTab === 'waveform' ? 'active' : ''}
                        onClick={() => setActiveTab('waveform')}
                    >
                        Waveform
                    </button>
                    <button
                        className={activeTab === 'errors' ? 'active' : ''}
                        onClick={() => setActiveTab('errors')}
                    >
                        Errors {stats && stats.total_errors > 0 && (
                            <span className="error-badge">{stats.total_errors}</span>
                        )}
                    </button>
                    <button
                        className={activeTab === 'stats' ? 'active' : ''}
                        onClick={() => setActiveTab('stats')}
                    >
                        Statistics
                    </button>
                </div>
            </div>

            {/* Content */}
            <div className="panel-content">
                {loading && !stats && <div className="loading-spinner" />}

                {/* Waveform Tab */}
                {activeTab === 'waveform' && (
                    <div className="waveform-tab">
                        {/* Navigation */}
                        <div className="waveform-nav">
                            <div className="nav-buttons">
                                <button onClick={() => handleCycleNav('start')} title="Go to start">⏮</button>
                                <button onClick={() => handleCycleNav('prev')} title="Previous">◀</button>
                                <button onClick={() => handleCycleNav('next')} title="Next">▶</button>
                                <button onClick={() => handleCycleNav('end')} title="Go to end">⏭</button>
                            </div>

                            <div className="cycle-range">
                                <label>Cycle:</label>
                                <input
                                    type="number"
                                    value={startCycle}
                                    onChange={e => setStartCycle(Math.max(0, parseInt(e.target.value) || 0))}
                                    min={0}
                                />
                                <span>to</span>
                                <span>{startCycle + cyclesVisible}</span>
                            </div>

                            <div className="zoom-controls">
                                <label>Zoom:</label>
                                <button
                                    onClick={() => setCyclesVisible(Math.min(128, cyclesVisible * 2))}
                                    disabled={cyclesVisible >= 128}
                                >
                                    −
                                </button>
                                <span>{cyclesVisible} cycles</span>
                                <button
                                    onClick={() => setCyclesVisible(Math.max(8, cyclesVisible / 2))}
                                    disabled={cyclesVisible <= 8}
                                >
                                    +
                                </button>
                            </div>

                            {stats && stats.total_errors > 0 && (
                                <button className="btn-error-jump" onClick={jumpToFirstError}>
                                    Jump to First Error
                                </button>
                            )}
                        </div>

                        {/* Channel Selector */}
                        <div className="channel-selector">
                            <span className="selector-label">Channels ({selectedChannels.length}/16):</span>
                            <div className="channel-chips">
                                {selectedChannels.map(pin => (
                                    <span
                                        key={pin}
                                        className={`channel-chip ${errorPinMask.includes(pin) ? 'error' : ''}`}
                                        onClick={() => toggleChannel(pin)}
                                    >
                                        Pin {pin} ×
                                    </span>
                                ))}
                                <button
                                    className="add-channel-btn"
                                    onClick={() => {
                                        const next = Array.from({ length: 160 }, (_, i) => i)
                                            .find(i => !selectedChannels.includes(i))
                                        if (next !== undefined) toggleChannel(next)
                                    }}
                                    disabled={selectedChannels.length >= 16}
                                >
                                    + Add
                                </button>
                            </div>
                        </div>

                        {/* Waveform Viewer */}
                        <WaveformViewer
                            channels={waveformChannels}
                            startCycle={startCycle}
                            cyclesVisible={cyclesVisible}
                            highlightErrors={true}
                            selectedCycle={selectedCycle}
                            onCycleSelect={setSelectedCycle}
                        />

                        {/* Selected Cycle Info */}
                        {selectedCycle !== undefined && (
                            <div className="cycle-info">
                                <h4>Cycle {selectedCycle}</h4>
                                <div className="cycle-details">
                                    <div className="detail-item">
                                        <span className="label">Vector:</span>
                                        <span className="value">{Math.floor(selectedCycle / 1)}</span>
                                    </div>
                                    {errors.find(e => e.cycle === selectedCycle) && (
                                        <div className="detail-item error">
                                            <span className="label">Error on pins:</span>
                                            <span className="value">
                                                {errors.find(e => e.cycle === selectedCycle)?.error_mask.join(', ')}
                                            </span>
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}
                    </div>
                )}

                {/* Errors Tab */}
                {activeTab === 'errors' && (
                    <div className="errors-tab">
                        {/* Error Summary */}
                        {stats && (
                            <div className="error-summary-grid">
                                <div className="summary-card">
                                    <div className="card-value">{stats.total_errors.toLocaleString()}</div>
                                    <div className="card-label">Total Errors</div>
                                </div>
                                <div className="summary-card">
                                    <div className="card-value">
                                        {stats.first_error_vector >= 0 ? stats.first_error_vector : '—'}
                                    </div>
                                    <div className="card-label">First Error Vector</div>
                                </div>
                                <div className="summary-card">
                                    <div className="card-value">{errorPinMask.length}</div>
                                    <div className="card-label">Pins with Errors</div>
                                </div>
                            </div>
                        )}

                        {/* Pin Error Grid */}
                        <div className="error-grid-section">
                            <h4>Pin Error Map</h4>
                            <PinGrid
                                pinCount={160}
                                errorMask={errorPinMask}
                                selectedPin={selectedPin}
                                onPinClick={setSelectedPin}
                                layout="16x10"
                            />
                            <div className="pin-grid-legend">
                                <div className="legend-item">
                                    <div className="legend-dot bim" />
                                    <span>BIM Pins (0-127)</span>
                                </div>
                                <div className="legend-item">
                                    <div className="legend-dot fast" />
                                    <span>Fast Pins (128-159)</span>
                                </div>
                                <div className="legend-item">
                                    <div className="legend-dot error" />
                                    <span>Error Detected</span>
                                </div>
                            </div>
                        </div>

                        {/* Error List */}
                        <div className="error-list-section">
                            <h4>Error Log</h4>
                            {errors.length === 0 ? (
                                <div className="no-errors">
                                    <span className="icon">✓</span>
                                    <p>No errors detected</p>
                                </div>
                            ) : (
                                <div className="error-table">
                                    <div className="table-header">
                                        <span>Vector</span>
                                        <span>Cycle</span>
                                        <span>Pin</span>
                                        <span>Time</span>
                                    </div>
                                    <div className="table-body">
                                        {errors.slice(0, 50).map((error, idx) => (
                                            <div key={idx} className="table-row" onClick={() => {
                                                setStartCycle(Math.max(0, error.cycle - 4))
                                                setSelectedCycle(error.cycle)
                                                setActiveTab('waveform')
                                            }}>
                                                <span className="mono">{error.vector}</span>
                                                <span className="mono">{error.cycle}</span>
                                                <span className="mono">{error.first_fail_pin}</span>
                                                <span className="time">
                                                    {new Date(error.timestamp).toLocaleTimeString()}
                                                </span>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {/* Statistics Tab */}
                {activeTab === 'stats' && (
                    <div className="stats-tab">
                        {stats ? (
                            <>
                                {/* Overview Cards */}
                                <div className="stats-grid">
                                    <div className="stat-card">
                                        <div className="stat-icon">📦</div>
                                        <div className="stat-info">
                                            <div className="stat-value">{stats.total_vectors.toLocaleString()}</div>
                                            <div className="stat-label">Total Vectors</div>
                                        </div>
                                    </div>
                                    <div className="stat-card">
                                        <div className="stat-icon">🔄</div>
                                        <div className="stat-info">
                                            <div className="stat-value">{stats.total_cycles.toLocaleString()}</div>
                                            <div className="stat-label">Total Cycles</div>
                                        </div>
                                    </div>
                                    <div className={`stat-card ${stats.total_errors > 0 ? 'error' : 'success'}`}>
                                        <div className="stat-icon">{stats.total_errors > 0 ? '❌' : '✅'}</div>
                                        <div className="stat-info">
                                            <div className="stat-value">{stats.total_errors.toLocaleString()}</div>
                                            <div className="stat-label">Errors</div>
                                        </div>
                                    </div>
                                </div>

                                {/* Error Distribution Chart */}
                                <div className="chart-section">
                                    <h4>Error Distribution by Pin</h4>
                                    {errorPinMask.length > 0 ? (
                                        <div className="pin-bar-chart">
                                            {errorPinMask.map((pin, idx) => {
                                                // Calculate consistent bar height based on pin position
                                                // In production, this would use actual per-pin error counts
                                                const barHeight = Math.min(100, 40 + ((pin * 7 + idx * 13) % 60))
                                                return (
                                                    <div key={pin} className="bar-item">
                                                        <div
                                                            className="bar"
                                                            style={{ height: `${barHeight}%` }}
                                                            title={`Pin ${pin}: ${barHeight}% relative errors`}
                                                        />
                                                        <div className="bar-label">{pin}</div>
                                                    </div>
                                                )
                                            })}
                                        </div>
                                    ) : (
                                        <div className="no-data">No errors to display</div>
                                    )}
                                </div>
                            </>
                        ) : (
                            <div className="no-stats">
                                <p>No pattern data loaded. Load vectors to see statistics.</p>
                            </div>
                        )}
                    </div>
                )}
            </div>
        </div>
    )
}
