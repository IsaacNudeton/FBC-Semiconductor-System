import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './AnalogMonitorPanel.css'

interface AnalogReading {
  raw: number
  voltage_mv: number
  name: string
}

interface AnalogChannelsResponse {
  xadc: AnalogReading[]
  external: AnalogReading[]
}

interface AnalogChannel {
  channel: number
  name: string
  value: number
  unit: string
  raw: number
  category: 'xadc' | 'external' | 'temp' | 'current'
}

// Channel definitions matching firmware
const CHANNEL_DEFS: Omit<AnalogChannel, 'value' | 'raw'>[] = [
  // XADC Internal (0-15)
  { channel: 0, name: 'DIE_TEMP', unit: '°C', category: 'temp' },
  { channel: 1, name: 'VCCINT', unit: 'mV', category: 'xadc' },
  { channel: 2, name: 'VCCAUX', unit: 'mV', category: 'xadc' },
  { channel: 3, name: 'VCCBRAM', unit: 'mV', category: 'xadc' },
  { channel: 4, name: 'VCCPINT', unit: 'mV', category: 'xadc' },
  { channel: 5, name: 'VCCPAUX', unit: 'mV', category: 'xadc' },
  { channel: 6, name: 'VCCO_DDR', unit: 'mV', category: 'xadc' },
  { channel: 7, name: 'VREFP', unit: 'mV', category: 'xadc' },
  { channel: 8, name: 'VREFN', unit: 'mV', category: 'xadc' },
  { channel: 9, name: 'XADC_AUX0', unit: 'mV', category: 'xadc' },
  { channel: 10, name: 'XADC_AUX1', unit: 'mV', category: 'xadc' },
  { channel: 11, name: 'XADC_AUX2', unit: 'mV', category: 'xadc' },
  { channel: 12, name: 'XADC_AUX3', unit: 'mV', category: 'xadc' },
  { channel: 13, name: 'XADC_AUX4', unit: 'mV', category: 'xadc' },
  { channel: 14, name: 'XADC_AUX5', unit: 'mV', category: 'xadc' },
  { channel: 15, name: 'XADC_AUX6', unit: 'mV', category: 'xadc' },
  // MAX11131 External (16-31)
  { channel: 16, name: 'VDD_CORE1', unit: 'mV', category: 'external' },
  { channel: 17, name: 'VDD_CORE2', unit: 'mV', category: 'external' },
  { channel: 18, name: 'VDD_CORE3', unit: 'mV', category: 'external' },
  { channel: 19, name: 'VDD_CORE4', unit: 'mV', category: 'external' },
  { channel: 20, name: 'VDD_CORE5', unit: 'mV', category: 'external' },
  { channel: 21, name: 'VDD_CORE6', unit: 'mV', category: 'external' },
  { channel: 22, name: 'THERM_CASE', unit: '°C', category: 'temp' },
  { channel: 23, name: 'THERM_DUT', unit: '°C', category: 'temp' },
  { channel: 24, name: 'I_CORE1', unit: 'mA', category: 'current' },
  { channel: 25, name: 'I_CORE2', unit: 'mA', category: 'current' },
  { channel: 26, name: 'VDD_IO', unit: 'mV', category: 'external' },
  { channel: 27, name: 'VDD_3V3', unit: 'mV', category: 'external' },
  { channel: 28, name: 'VDD_5V', unit: 'mV', category: 'external' },
  { channel: 29, name: 'VDD_12V', unit: 'mV', category: 'external' },
  { channel: 30, name: 'VREF_4V096', unit: 'mV', category: 'external' },
  { channel: 31, name: 'AUX_ADC', unit: 'mV', category: 'external' },
]

export default function AnalogMonitorPanel() {
  const { selectedBoard, connected } = useStore()
  const [channels, setChannels] = useState<AnalogChannel[]>([])
  const [refreshRate, setRefreshRate] = useState(1000) // ms
  const [filter, setFilter] = useState<'all' | 'xadc' | 'external' | 'temp' | 'current'>('all')
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid')

  useEffect(() => {
    if (!selectedBoard || !connected) {
      setChannels([])
      return
    }

    const fetchChannels = async () => {
      try {
        const result = await invoke<AnalogChannelsResponse>('read_analog_channels', { mac: selectedBoard })
        const updated: AnalogChannel[] = []

        // Map XADC channels (0-15)
        for (let i = 0; i < 16 && i < result.xadc.length; i++) {
          const reading = result.xadc[i]
          const def = CHANNEL_DEFS[i]
          if (def) {
            updated.push({
              ...def,
              name: reading.name || def.name,
              value: def.unit === '°C' ? reading.voltage_mv : reading.voltage_mv,
              raw: reading.raw,
            })
          }
        }

        // Map external ADC channels (16-31)
        for (let i = 0; i < 16 && i < result.external.length; i++) {
          const reading = result.external[i]
          const def = CHANNEL_DEFS[16 + i]
          if (def) {
            updated.push({
              ...def,
              name: reading.name || def.name,
              value: reading.voltage_mv,
              raw: reading.raw,
            })
          }
        }

        setChannels(updated)
      } catch (e) {
        console.error('Failed to read analog channels:', e)
      }
    }

    fetchChannels()
    const interval = setInterval(fetchChannels, refreshRate)
    return () => clearInterval(interval)
  }, [selectedBoard, connected, refreshRate])

  const filteredChannels = filter === 'all'
    ? channels
    : channels.filter(c => c.category === filter)

  const getCategoryColor = (category: string): string => {
    switch (category) {
      case 'xadc': return '#3b82f6'
      case 'external': return '#10b981'
      case 'temp': return '#f59e0b'
      case 'current': return '#8b5cf6'
      default: return '#6b7280'
    }
  }

  const getValueColor = (ch: AnalogChannel): string => {
    if (ch.unit === '°C') {
      if (ch.value > 85) return 'var(--error)'
      if (ch.value > 70) return '#f59e0b'
      return 'var(--success)'
    }
    if (ch.unit === 'mV' && ch.name.includes('VCC')) {
      // Check if voltage is within typical range
      const nominal = ch.name === 'VCCINT' ? 1000 : ch.name === 'VCCAUX' ? 1800 : 1000
      const deviation = Math.abs(ch.value - nominal) / nominal
      if (deviation > 0.1) return 'var(--error)'
      if (deviation > 0.05) return '#f59e0b'
      return 'var(--success)'
    }
    return 'var(--text-primary)'
  }

  if (!selectedBoard) {
    return (
      <div className="analog-monitor-panel">
        <div className="no-board-message">
          <span className="icon">📈</span>
          <h3>No Board Selected</h3>
          <p>Select a board to view analog measurements.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="analog-monitor-panel">
      {/* Header */}
      <div className="monitor-header">
        <div className="header-title">
          <h2>Analog Monitor</h2>
          <span className="channel-count">{channels.length} channels</span>
        </div>
        <div className="header-controls">
          <div className="refresh-control">
            <label>Refresh:</label>
            <select value={refreshRate} onChange={e => setRefreshRate(Number(e.target.value))}>
              <option value={500}>500ms</option>
              <option value={1000}>1s</option>
              <option value={2000}>2s</option>
              <option value={5000}>5s</option>
            </select>
          </div>
          <div className="view-toggle">
            <button
              className={viewMode === 'grid' ? 'active' : ''}
              onClick={() => setViewMode('grid')}
              title="Grid View"
            >
              ⊞
            </button>
            <button
              className={viewMode === 'list' ? 'active' : ''}
              onClick={() => setViewMode('list')}
              title="List View"
            >
              ≡
            </button>
          </div>
        </div>
      </div>

      {/* Filter Tabs */}
      <div className="filter-tabs">
        <button
          className={filter === 'all' ? 'active' : ''}
          onClick={() => setFilter('all')}
        >
          All ({channels.length})
        </button>
        <button
          className={filter === 'xadc' ? 'active' : ''}
          onClick={() => setFilter('xadc')}
          style={{ borderColor: getCategoryColor('xadc') }}
        >
          XADC ({channels.filter(c => c.category === 'xadc').length})
        </button>
        <button
          className={filter === 'external' ? 'active' : ''}
          onClick={() => setFilter('external')}
          style={{ borderColor: getCategoryColor('external') }}
        >
          External ({channels.filter(c => c.category === 'external').length})
        </button>
        <button
          className={filter === 'temp' ? 'active' : ''}
          onClick={() => setFilter('temp')}
          style={{ borderColor: getCategoryColor('temp') }}
        >
          Temp ({channels.filter(c => c.category === 'temp').length})
        </button>
        <button
          className={filter === 'current' ? 'active' : ''}
          onClick={() => setFilter('current')}
          style={{ borderColor: getCategoryColor('current') }}
        >
          Current ({channels.filter(c => c.category === 'current').length})
        </button>
      </div>

      {/* Channel Display */}
      <div className={`channel-container ${viewMode}`}>
        {filteredChannels.map(ch => (
          <div
            key={ch.channel}
            className="channel-card"
            style={{ borderLeftColor: getCategoryColor(ch.category) }}
          >
            <div className="channel-header">
              <span className="channel-num">CH{ch.channel}</span>
              <span className="channel-name">{ch.name}</span>
            </div>
            <div className="channel-value" style={{ color: getValueColor(ch) }}>
              {ch.value.toFixed(ch.unit === '°C' ? 1 : 0)}
              <span className="channel-unit">{ch.unit}</span>
            </div>
            {viewMode === 'list' && (
              <div className="channel-bar">
                <div
                  className="channel-bar-fill"
                  style={{
                    width: `${Math.min(100, (ch.value / (ch.unit === '°C' ? 100 : ch.unit === 'mA' ? 50000 : 5000)) * 100)}%`,
                    background: getCategoryColor(ch.category),
                  }}
                />
              </div>
            )}
            <div className="channel-raw">
              Raw: {ch.raw}
            </div>
          </div>
        ))}
      </div>

      {/* Summary Cards */}
      <div className="summary-section">
        <div className="summary-card">
          <div className="summary-icon" style={{ background: getCategoryColor('temp') }}>🌡️</div>
          <div className="summary-content">
            <div className="summary-label">Max Temperature</div>
            <div className="summary-value">
              {Math.max(...channels.filter(c => c.category === 'temp').map(c => c.value)).toFixed(1)}°C
            </div>
          </div>
        </div>
        <div className="summary-card">
          <div className="summary-icon" style={{ background: getCategoryColor('xadc') }}>⚡</div>
          <div className="summary-content">
            <div className="summary-label">VCCINT</div>
            <div className="summary-value">
              {channels.find(c => c.name === 'VCCINT')?.value.toFixed(0) || '—'} mV
            </div>
          </div>
        </div>
        <div className="summary-card">
          <div className="summary-icon" style={{ background: getCategoryColor('current') }}>🔌</div>
          <div className="summary-content">
            <div className="summary-label">Total Current</div>
            <div className="summary-value">
              {channels.filter(c => c.category === 'current').reduce((sum, c) => sum + c.value, 0).toFixed(0)} mA
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
