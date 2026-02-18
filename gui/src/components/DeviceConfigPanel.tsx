import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useStore } from '../store'
import './DeviceConfigPanel.css'

// Device profile files that make up a complete config
interface DeviceProfile {
  name: string
  bimFile: string | null      // .bim - BIM XML definition
  mapFile: string | null      // .map - Pin mapping
  timFile: string | null      // .tim - Timing config
  lvlFile: string | null      // .lvl - Level settings
  tpFile: string | null       // .tp/.tpf - Test plan
}

interface PinConfig {
  pin: number
  name: string
  type: 'bidi' | 'input' | 'output' | 'open_c' | 'pulse' | 'npulse' | 'error_trig' | 'vec_clk' | 'clk_en'
  bank: number
  group: string
}

interface TimingConfig {
  setup_ns: number
  hold_ns: number
  strobe_ns: number
  period_ns: number
}

interface DeviceConfig {
  name: string
  type: number
  version: number
  pins: PinConfig[]
  timing: TimingConfig
  loaded: boolean
}

const PIN_TYPE_NAMES: Record<string, string> = {
  bidi: 'Bidirectional',
  input: 'Input (Compare)',
  output: 'Output (Drive)',
  open_c: 'Open Collector',
  pulse: 'Pulse',
  npulse: 'Inverted Pulse',
  error_trig: 'Error Trigger',
  vec_clk: 'Vector Clock',
  clk_en: 'Clock Enable',
}

export default function DeviceConfigPanel() {
  const { selectedBoard, connected } = useStore()
  const [config, setConfig] = useState<DeviceConfig | null>(null)
  const [activeTab, setActiveTab] = useState<'overview' | 'pins' | 'timing' | 'profile'>('profile')
  const [pinFilter, setPinFilter] = useState<string>('all')
  const [profile, setProfile] = useState<DeviceProfile>({
    name: '',
    bimFile: null,
    mapFile: null,
    timFile: null,
    lvlFile: null,
    tpFile: null,
  })
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Load a device file
  const loadFile = async (fileType: keyof DeviceProfile) => {
    const extensions: Record<string, string[]> = {
      bimFile: ['bim'],
      mapFile: ['map'],
      timFile: ['tim'],
      lvlFile: ['lvl'],
      tpFile: ['tp', 'tpf'],
    }

    const ext = extensions[fileType]
    if (!ext) return

    const file = await open({
      multiple: false,
      filters: [{ name: `${fileType.replace('File', '').toUpperCase()} Files`, extensions: ext }],
    })

    if (file) {
      setProfile(prev => ({ ...prev, [fileType]: file }))

      // Extract device name from BIM file path
      if (fileType === 'bimFile') {
        const name = file.split(/[/\\]/).pop()?.replace('.bim', '') ?? ''
        setProfile(prev => ({ ...prev, name }))
      }
    }
  }

  // Compile and load the device configuration
  const compileProfile = async () => {
    if (!profile.bimFile) {
      setError('BIM file is required')
      return
    }

    setLoading(true)
    setError(null)

    try {
      const result = await invoke<DeviceConfig>('compile_device_config', {
        bimPath: profile.bimFile,
        mapPath: profile.mapFile,
        timPath: profile.timFile,
      })
      setConfig(result)
      setActiveTab('overview')
    } catch (e) {
      setError(`Failed to compile: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (!selectedBoard || !connected) {
      setConfig(null)
      return
    }
    // Don't auto-load mock data anymore - wait for user to load profile
  }, [selectedBoard, connected])

  const filteredPins = config?.pins.filter(p => {
    if (pinFilter === 'all') return true
    return p.type === pinFilter
  }) || []

  const pinTypeCounts = config?.pins.reduce((acc, p) => {
    acc[p.type] = (acc[p.type] || 0) + 1
    return acc
  }, {} as Record<string, number>) || {}

  if (!selectedBoard) {
    return (
      <div className="device-config-panel">
        <div className="no-board-message">
          <span className="icon">⚙️</span>
          <h3>No Board Selected</h3>
          <p>Select a board to view device configuration.</p>
        </div>
      </div>
    )
  }

  if (!config) {
    return (
      <div className="device-config-panel">
        <div className="no-config-message">
          <span className="icon">📄</span>
          <h3>No Device Configuration</h3>
          <p>Load a device configuration file (.fbc) to configure this board.</p>
          <button className="btn-primary" onClick={() => {}}>
            Load Configuration
          </button>
        </div>
      </div>
    )
  }

  // Export config as JSON file
  const handleExport = () => {
    if (!config) return

    const exportData = {
      name: config.name,
      type: config.type,
      version: config.version,
      timing: config.timing,
      pins: config.pins,
      exported_at: new Date().toISOString(),
      board_mac: selectedBoard,
    }

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${config.name.replace(/\s+/g, '_')}_config.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  return (
    <div className="device-config-panel">
      {/* Header */}
      <div className="config-header">
        <div className="header-info">
          <h2>Device Configuration</h2>
          <div className="config-summary">
            <span className="device-name">{config.name}</span>
            <span className="device-type">Type: {config.type}</span>
            <span className="pin-count">{config.pins.length} pins</span>
          </div>
        </div>
        <div className="header-actions">
          <button className="btn-load">Load Config</button>
          <button className="btn-export" onClick={handleExport}>Export</button>
        </div>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {/* Tabs */}
      <div className="config-tabs">
        <button
          className={activeTab === 'profile' ? 'active' : ''}
          onClick={() => setActiveTab('profile')}
        >
          Device Profile
        </button>
        <button
          className={activeTab === 'overview' ? 'active' : ''}
          onClick={() => setActiveTab('overview')}
          disabled={!config}
        >
          Overview
        </button>
        <button
          className={activeTab === 'pins' ? 'active' : ''}
          onClick={() => setActiveTab('pins')}
          disabled={!config}
        >
          Pin Configuration
        </button>
        <button
          className={activeTab === 'timing' ? 'active' : ''}
          onClick={() => setActiveTab('timing')}
          disabled={!config}
        >
          Timing
        </button>
      </div>

      {/* Content */}
      <div className="config-content">
        {activeTab === 'profile' && (
          <div className="profile-content">
            <h3>Device Profile Files</h3>
            <p className="profile-description">
              Load Sonoma device files to configure the board. At minimum, a .bim file is required.
            </p>

            <div className="profile-files">
              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.bim</span>
                  <span className="file-desc">BIM definition (required)</span>
                </div>
                <div className="file-value">
                  {profile.bimFile ? (
                    <span className="file-path">{profile.bimFile.split(/[/\\]/).pop()}</span>
                  ) : (
                    <span className="file-empty">Not loaded</span>
                  )}
                </div>
                <button className="btn-browse" onClick={() => loadFile('bimFile')}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.map</span>
                  <span className="file-desc">Pin mapping</span>
                </div>
                <div className="file-value">
                  {profile.mapFile ? (
                    <span className="file-path">{profile.mapFile.split(/[/\\]/).pop()}</span>
                  ) : (
                    <span className="file-empty">Not loaded</span>
                  )}
                </div>
                <button className="btn-browse" onClick={() => loadFile('mapFile')}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.tim</span>
                  <span className="file-desc">Timing config</span>
                </div>
                <div className="file-value">
                  {profile.timFile ? (
                    <span className="file-path">{profile.timFile.split(/[/\\]/).pop()}</span>
                  ) : (
                    <span className="file-empty">Not loaded</span>
                  )}
                </div>
                <button className="btn-browse" onClick={() => loadFile('timFile')}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.lvl</span>
                  <span className="file-desc">Level settings</span>
                </div>
                <div className="file-value">
                  {profile.lvlFile ? (
                    <span className="file-path">{profile.lvlFile.split(/[/\\]/).pop()}</span>
                  ) : (
                    <span className="file-empty">Not loaded</span>
                  )}
                </div>
                <button className="btn-browse" onClick={() => loadFile('lvlFile')}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.tp</span>
                  <span className="file-desc">Test plan</span>
                </div>
                <div className="file-value">
                  {profile.tpFile ? (
                    <span className="file-path">{profile.tpFile.split(/[/\\]/).pop()}</span>
                  ) : (
                    <span className="file-empty">Not loaded</span>
                  )}
                </div>
                <button className="btn-browse" onClick={() => loadFile('tpFile')}>Browse</button>
              </div>
            </div>

            <div className="profile-actions">
              <button
                className="btn-compile"
                onClick={compileProfile}
                disabled={!profile.bimFile || loading}
              >
                {loading ? 'Compiling...' : 'Compile & Load'}
              </button>
              <button
                className="btn-clear"
                onClick={() => setProfile({ name: '', bimFile: null, mapFile: null, timFile: null, lvlFile: null, tpFile: null })}
              >
                Clear All
              </button>
            </div>

            {profile.name && (
              <div className="profile-summary">
                <span className="summary-label">Device:</span>
                <span className="summary-value">{profile.name}</span>
              </div>
            )}
          </div>
        )}

        {activeTab === 'overview' && config && (
          <div className="overview-content">
            <div className="info-cards">
              <div className="info-card">
                <div className="card-icon">📦</div>
                <div className="card-content">
                  <span className="card-label">Device</span>
                  <span className="card-value">{config.name}</span>
                </div>
              </div>
              <div className="info-card">
                <div className="card-icon">🔢</div>
                <div className="card-content">
                  <span className="card-label">Type Code</span>
                  <span className="card-value">{config.type}</span>
                </div>
              </div>
              <div className="info-card">
                <div className="card-icon">📌</div>
                <div className="card-content">
                  <span className="card-label">Total Pins</span>
                  <span className="card-value">{config.pins.length}</span>
                </div>
              </div>
              <div className="info-card">
                <div className="card-icon">⏱️</div>
                <div className="card-content">
                  <span className="card-label">Period</span>
                  <span className="card-value">{config.timing.period_ns} ns</span>
                </div>
              </div>
            </div>

            <div className="pin-breakdown">
              <h3>Pin Type Distribution</h3>
              <div className="breakdown-grid">
                {Object.entries(pinTypeCounts).map(([type, count]) => (
                  <div key={type} className="breakdown-item">
                    <div className="breakdown-bar">
                      <div
                        className="breakdown-fill"
                        style={{ width: `${(count / config.pins.length) * 100}%` }}
                      />
                    </div>
                    <div className="breakdown-info">
                      <span className="breakdown-type">{PIN_TYPE_NAMES[type] || type}</span>
                      <span className="breakdown-count">{count}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {activeTab === 'pins' && config && (
          <div className="pins-content">
            <div className="pin-filters">
              <button
                className={pinFilter === 'all' ? 'active' : ''}
                onClick={() => setPinFilter('all')}
              >
                All ({config.pins.length})
              </button>
              {Object.entries(pinTypeCounts).map(([type, count]) => (
                <button
                  key={type}
                  className={pinFilter === type ? 'active' : ''}
                  onClick={() => setPinFilter(type)}
                >
                  {PIN_TYPE_NAMES[type] || type} ({count})
                </button>
              ))}
            </div>

            <div className="pin-table">
              <div className="pin-header">
                <span>Pin</span>
                <span>Name</span>
                <span>Type</span>
                <span>Bank</span>
              </div>
              <div className="pin-list">
                {filteredPins.map(pin => (
                  <div key={pin.pin} className="pin-row">
                    <span className="pin-num">{pin.pin}</span>
                    <span className="pin-name">{pin.name}</span>
                    <span className={`pin-type type-${pin.type}`}>
                      {PIN_TYPE_NAMES[pin.type] || pin.type}
                    </span>
                    <span className="pin-bank">{pin.bank}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {activeTab === 'timing' && config && (
          <div className="timing-content">
            <div className="timing-diagram">
              <h3>Timing Parameters</h3>
              <div className="timing-visual">
                <div className="timing-waveform">
                  <svg viewBox="0 0 400 100" className="waveform-svg">
                    {/* Clock signal */}
                    <path
                      d="M0,70 L50,70 L50,30 L150,30 L150,70 L250,70 L250,30 L350,30 L350,70 L400,70"
                      fill="none"
                      stroke="var(--accent)"
                      strokeWidth="2"
                    />
                    {/* Setup time indicator */}
                    <line x1="50" y1="85" x2="100" y2="85" stroke="var(--success)" strokeWidth="2" />
                    <text x="75" y="98" fill="var(--text-secondary)" fontSize="10" textAnchor="middle">Setup</text>
                    {/* Hold time indicator */}
                    <line x1="150" y1="85" x2="200" y2="85" stroke="var(--warning)" strokeWidth="2" />
                    <text x="175" y="98" fill="var(--text-secondary)" fontSize="10" textAnchor="middle">Hold</text>
                    {/* Strobe indicator */}
                    <line x1="100" y1="50" x2="100" y2="70" stroke="var(--error)" strokeWidth="2" strokeDasharray="4" />
                    <text x="100" y="45" fill="var(--text-secondary)" fontSize="10" textAnchor="middle">Strobe</text>
                  </svg>
                </div>
              </div>
            </div>

            <div className="timing-params">
              <div className="timing-param">
                <label>Setup Time</label>
                <div className="param-value">
                  <input type="number" value={config.timing.setup_ns} readOnly />
                  <span className="unit">ns</span>
                </div>
              </div>
              <div className="timing-param">
                <label>Hold Time</label>
                <div className="param-value">
                  <input type="number" value={config.timing.hold_ns} readOnly />
                  <span className="unit">ns</span>
                </div>
              </div>
              <div className="timing-param">
                <label>Strobe Point</label>
                <div className="param-value">
                  <input type="number" value={config.timing.strobe_ns} readOnly />
                  <span className="unit">ns</span>
                </div>
              </div>
              <div className="timing-param">
                <label>Period</label>
                <div className="param-value">
                  <input type="number" value={config.timing.period_ns} readOnly />
                  <span className="unit">ns</span>
                </div>
              </div>
            </div>

            <div className="timing-derived">
              <h4>Derived Parameters</h4>
              <div className="derived-grid">
                <div className="derived-item">
                  <span className="derived-label">Frequency</span>
                  <span className="derived-value">{(1000 / config.timing.period_ns).toFixed(1)} MHz</span>
                </div>
                <div className="derived-item">
                  <span className="derived-label">Duty Cycle</span>
                  <span className="derived-value">50%</span>
                </div>
                <div className="derived-item">
                  <span className="derived-label">Max Vectors/sec</span>
                  <span className="derived-value">{((1000000000 / config.timing.period_ns) / 1000000).toFixed(1)}M</span>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
