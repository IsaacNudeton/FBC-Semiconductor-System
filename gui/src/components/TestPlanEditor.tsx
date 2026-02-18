import { useState, useEffect, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import './TestPlanEditor.css'

// =============================================================================
// FBC Test Plan Types (from OPERATIONAL_WORKFLOW.md)
// =============================================================================

export interface VectorConfig {
  file: string
  checksum?: string
  vector_count?: number
  compression_ratio?: number
}

export interface DeviceConfig {
  file: string
  checksum?: string
  part_number?: string
  pin_count?: number
}

export interface ExecutionConfig {
  loops: number
  loop_mode: 'continuous' | 'fixed'
  stop_on_error: boolean
  error_threshold: number
}

export interface PowerRail {
  id: number
  name: string
  voltage_mv: number
  current_limit_ma: number
}

export interface PowerConfig {
  sequence: 'standard' | 'custom'
  rails: PowerRail[]
  ramp_rate_mv_per_ms: number
}

export interface ThermalConfig {
  setpoint_c: number
  tolerance_c: number
  soak_time_s: number
}

export interface TimingConfig {
  vector_clock_mhz: number
  setup_ns: number
  hold_ns: number
}

export interface BoardFilter {
  mode: 'all' | 'rack' | 'slots' | 'macs'
  rack?: number
  slots?: number[]
  macs?: string[]
}

export interface FbcTestPlan {
  $schema: string
  name: string
  description: string
  version: string
  created: string
  vectors: VectorConfig
  device: DeviceConfig
  execution: ExecutionConfig
  power: PowerConfig
  thermal: ThermalConfig
  timing: TimingConfig
  board_filter: BoardFilter
}

// =============================================================================
// Defaults
// =============================================================================

const createEmptyTestPlan = (): FbcTestPlan => ({
  $schema: 'fbc-test-plan-v1',
  name: 'New Test Plan',
  description: '',
  version: '1.0.0',
  created: new Date().toISOString(),
  vectors: {
    file: '',
  },
  device: {
    file: '',
  },
  execution: {
    loops: 1000,
    loop_mode: 'continuous',
    stop_on_error: false,
    error_threshold: 100,
  },
  power: {
    sequence: 'standard',
    rails: [
      { id: 0, name: 'VDD', voltage_mv: 1200, current_limit_ma: 2000 },
      { id: 1, name: 'VDDQ', voltage_mv: 1200, current_limit_ma: 500 },
    ],
    ramp_rate_mv_per_ms: 10,
  },
  thermal: {
    setpoint_c: 85,
    tolerance_c: 2,
    soak_time_s: 300,
  },
  timing: {
    vector_clock_mhz: 100,
    setup_ns: 2,
    hold_ns: 2,
  },
  board_filter: {
    mode: 'all',
  },
})

// =============================================================================
// Component
// =============================================================================

export default function TestPlanEditor() {
  const [testPlan, setTestPlan] = useState<FbcTestPlan>(createEmptyTestPlan())
  const [activeSection, setActiveSection] = useState<'general' | 'files' | 'execution' | 'power' | 'thermal' | 'preview'>('general')
  const [modified, setModified] = useState(false)
  const [filePath, setFilePath] = useState<string | null>(null)
  const [validationErrors, setValidationErrors] = useState<string[]>([])

  // Validate test plan
  const validatePlan = useMemo(() => {
    const errors: string[] = []

    if (!testPlan.name.trim()) {
      errors.push('Test plan name is required')
    }
    if (!testPlan.vectors.file.trim()) {
      errors.push('Vector file path is required')
    }
    if (!testPlan.device.file.trim()) {
      errors.push('Device config file path is required')
    }
    if (testPlan.execution.loops <= 0) {
      errors.push('Loop count must be greater than 0')
    }
    if (testPlan.power.rails.length === 0) {
      errors.push('At least one power rail is required')
    }
    testPlan.power.rails.forEach((rail) => {
      if (rail.voltage_mv <= 0) {
        errors.push(`Rail ${rail.name}: Voltage must be greater than 0`)
      }
      if (rail.current_limit_ma <= 0) {
        errors.push(`Rail ${rail.name}: Current limit must be greater than 0`)
      }
    })

    return errors
  }, [testPlan])

  useEffect(() => {
    setValidationErrors(validatePlan)
  }, [validatePlan])

  // Update nested field
  const updateField = (
    section: keyof FbcTestPlan,
    field: string,
    value: any
  ) => {
    setTestPlan(prev => ({
      ...prev,
      [section]: { ...(prev[section] as object), [field]: value }
    }))
    setModified(true)
  }

  // Update top-level field
  const updateTopLevel = (field: keyof FbcTestPlan, value: any) => {
    setTestPlan(prev => ({ ...prev, [field]: value }))
    setModified(true)
  }

  // Power rail management
  const addRail = () => {
    const newId = testPlan.power.rails.length
    setTestPlan(prev => ({
      ...prev,
      power: {
        ...prev.power,
        rails: [...prev.power.rails, {
          id: newId,
          name: `RAIL_${newId}`,
          voltage_mv: 1200,
          current_limit_ma: 1000,
        }]
      }
    }))
    setModified(true)
  }

  const removeRail = (index: number) => {
    setTestPlan(prev => ({
      ...prev,
      power: {
        ...prev.power,
        rails: prev.power.rails.filter((_, i) => i !== index)
      }
    }))
    setModified(true)
  }

  const updateRail = (index: number, field: keyof PowerRail, value: any) => {
    setTestPlan(prev => ({
      ...prev,
      power: {
        ...prev.power,
        rails: prev.power.rails.map((r, i) =>
          i === index ? { ...r, [field]: value } : r
        )
      }
    }))
    setModified(true)
  }

  // Generate JSON
  const generateJson = (): string => {
    return JSON.stringify(testPlan, null, 2)
  }

  // File operations
  const handleNew = () => {
    if (modified && !confirm('Discard unsaved changes?')) return
    setTestPlan(createEmptyTestPlan())
    setFilePath(null)
    setModified(false)
  }

  const handleSave = async () => {
    try {
      let path = filePath
      if (!path) {
        const result = await save({
          defaultPath: `${testPlan.name.replace(/\s+/g, '_')}.json`,
          filters: [{ name: 'FBC Test Plan', extensions: ['json'] }]
        })
        if (!result) return
        path = result
      }

      await invoke('write_file', { path, content: generateJson() })
      setFilePath(path)
      setModified(false)
    } catch (e) {
      console.error('Failed to save:', e)
      alert(`Save failed: ${e}`)
    }
  }

  const handleLoad = async () => {
    if (modified && !confirm('Discard unsaved changes?')) return

    try {
      const result = await open({
        filters: [{ name: 'FBC Test Plan', extensions: ['json'] }],
        multiple: false,
      })
      if (!result) return

      const content = await invoke<string>('read_file', { path: result })
      const parsed = JSON.parse(content) as FbcTestPlan

      // Validate schema
      if (parsed.$schema !== 'fbc-test-plan-v1') {
        alert('Invalid test plan format')
        return
      }

      setTestPlan(parsed)
      setFilePath(result as string)
      setModified(false)
    } catch (e) {
      console.error('Failed to load:', e)
      alert(`Load failed: ${e}`)
    }
  }

  // Browse for files
  const browseVectors = async () => {
    const result = await open({
      filters: [{ name: 'FBC Vectors', extensions: ['fbc'] }],
      multiple: false,
    })
    if (result) {
      updateField('vectors', 'file', result)
    }
  }

  const browseDevice = async () => {
    const result = await open({
      filters: [{ name: 'FBC Device Config', extensions: ['fbcfg'] }],
      multiple: false,
    })
    if (result) {
      updateField('device', 'file', result)
    }
  }

  return (
    <div className="test-plan-editor">
      {/* Header */}
      <div className="editor-header">
        <div className="editor-title">
          <h2>Test Plan Editor</h2>
          {filePath && <span className="file-path">{filePath}</span>}
          {modified && <span className="modified-badge">Modified</span>}
        </div>
        <div className="editor-actions">
          <button className="btn btn-secondary" onClick={handleNew}>New</button>
          <button className="btn btn-secondary" onClick={handleLoad}>Open</button>
          <button className="btn btn-primary" onClick={handleSave}>
            {filePath ? 'Save' : 'Save As'}
          </button>
        </div>
      </div>

      {/* Validation Errors */}
      {validationErrors.length > 0 && (
        <div className="validation-errors">
          {validationErrors.map((err, i) => (
            <div key={i} className="error-item">{err}</div>
          ))}
        </div>
      )}

      {/* Section Tabs */}
      <div className="section-tabs">
        <button
          className={activeSection === 'general' ? 'active' : ''}
          onClick={() => setActiveSection('general')}
        >
          General
        </button>
        <button
          className={activeSection === 'files' ? 'active' : ''}
          onClick={() => setActiveSection('files')}
        >
          Files
        </button>
        <button
          className={activeSection === 'execution' ? 'active' : ''}
          onClick={() => setActiveSection('execution')}
        >
          Execution
        </button>
        <button
          className={activeSection === 'power' ? 'active' : ''}
          onClick={() => setActiveSection('power')}
        >
          Power ({testPlan.power.rails.length} rails)
        </button>
        <button
          className={activeSection === 'thermal' ? 'active' : ''}
          onClick={() => setActiveSection('thermal')}
        >
          Thermal
        </button>
        <button
          className={activeSection === 'preview' ? 'active' : ''}
          onClick={() => setActiveSection('preview')}
        >
          JSON Preview
        </button>
      </div>

      {/* Content */}
      <div className="editor-content">
        {activeSection === 'general' && (
          <div className="section-general">
            <div className="info-section">
              <h4>Test Plan Information</h4>
              <div className="form-grid">
                <div className="form-field">
                  <label>Name *</label>
                  <input
                    type="text"
                    value={testPlan.name}
                    onChange={e => updateTopLevel('name', e.target.value)}
                    placeholder="DDR4 Burn-in Stress Test"
                  />
                </div>
                <div className="form-field">
                  <label>Version</label>
                  <input
                    type="text"
                    value={testPlan.version}
                    onChange={e => updateTopLevel('version', e.target.value)}
                    placeholder="1.0.0"
                  />
                </div>
                <div className="form-field full-width">
                  <label>Description</label>
                  <textarea
                    value={testPlan.description}
                    onChange={e => updateTopLevel('description', e.target.value)}
                    placeholder="High-temperature stress with March-C pattern"
                    rows={3}
                  />
                </div>
              </div>
            </div>

            <div className="info-section">
              <h4>Board Filter</h4>
              <div className="form-grid">
                <div className="form-field">
                  <label>Mode</label>
                  <select
                    value={testPlan.board_filter.mode}
                    onChange={e => updateField('board_filter', 'mode', e.target.value)}
                  >
                    <option value="all">All Boards</option>
                    <option value="rack">By Rack</option>
                    <option value="slots">By Slot</option>
                    <option value="macs">By MAC Address</option>
                  </select>
                </div>
                {testPlan.board_filter.mode === 'rack' && (
                  <div className="form-field">
                    <label>Rack Number</label>
                    <input
                      type="number"
                      min="1"
                      value={testPlan.board_filter.rack || ''}
                      onChange={e => updateField('board_filter', 'rack', parseInt(e.target.value) || null)}
                    />
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {activeSection === 'files' && (
          <div className="section-files">
            <div className="info-section">
              <h4>Vector File</h4>
              <p className="section-desc">Compiled .fbc vector file containing test patterns</p>
              <div className="form-grid">
                <div className="form-field full-width">
                  <label>Vector File Path *</label>
                  <div className="file-input">
                    <input
                      type="text"
                      value={testPlan.vectors.file}
                      onChange={e => updateField('vectors', 'file', e.target.value)}
                      placeholder="vectors/ddr4_march_c.fbc"
                    />
                    <button className="btn btn-small" onClick={browseVectors}>Browse</button>
                  </div>
                </div>
                <div className="form-field">
                  <label>Checksum (optional)</label>
                  <input
                    type="text"
                    value={testPlan.vectors.checksum || ''}
                    onChange={e => updateField('vectors', 'checksum', e.target.value || undefined)}
                    placeholder="a1b2c3d4"
                  />
                </div>
                <div className="form-field">
                  <label>Vector Count (optional)</label>
                  <input
                    type="number"
                    value={testPlan.vectors.vector_count || ''}
                    onChange={e => updateField('vectors', 'vector_count', parseInt(e.target.value) || undefined)}
                    placeholder="50000"
                  />
                </div>
              </div>
            </div>

            <div className="info-section">
              <h4>Device Configuration</h4>
              <p className="section-desc">Compiled .fbcfg file with pin types and timing</p>
              <div className="form-grid">
                <div className="form-field full-width">
                  <label>Device Config File Path *</label>
                  <div className="file-input">
                    <input
                      type="text"
                      value={testPlan.device.file}
                      onChange={e => updateField('device', 'file', e.target.value)}
                      placeholder="device-configs/ddr4_x16.fbcfg"
                    />
                    <button className="btn btn-small" onClick={browseDevice}>Browse</button>
                  </div>
                </div>
                <div className="form-field">
                  <label>Part Number (optional)</label>
                  <input
                    type="text"
                    value={testPlan.device.part_number || ''}
                    onChange={e => updateField('device', 'part_number', e.target.value || undefined)}
                    placeholder="MT40A1G16"
                  />
                </div>
                <div className="form-field">
                  <label>Pin Count (optional)</label>
                  <input
                    type="number"
                    value={testPlan.device.pin_count || ''}
                    onChange={e => updateField('device', 'pin_count', parseInt(e.target.value) || undefined)}
                    placeholder="96"
                  />
                </div>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'execution' && (
          <div className="section-execution">
            <div className="info-section">
              <h4>Execution Settings</h4>
              <div className="form-grid">
                <div className="form-field">
                  <label>Loop Count *</label>
                  <input
                    type="number"
                    min="1"
                    value={testPlan.execution.loops}
                    onChange={e => updateField('execution', 'loops', parseInt(e.target.value) || 1)}
                  />
                </div>
                <div className="form-field">
                  <label>Loop Mode</label>
                  <select
                    value={testPlan.execution.loop_mode}
                    onChange={e => updateField('execution', 'loop_mode', e.target.value)}
                  >
                    <option value="continuous">Continuous</option>
                    <option value="fixed">Fixed Count</option>
                  </select>
                </div>
                <div className="form-field">
                  <label>Error Threshold</label>
                  <input
                    type="number"
                    min="0"
                    value={testPlan.execution.error_threshold}
                    onChange={e => updateField('execution', 'error_threshold', parseInt(e.target.value) || 0)}
                  />
                  <span className="field-hint">Stop after this many errors (0 = unlimited)</span>
                </div>
                <div className="form-field">
                  <label>Stop on Error</label>
                  <label className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={testPlan.execution.stop_on_error}
                      onChange={e => updateField('execution', 'stop_on_error', e.target.checked)}
                    />
                    Stop immediately on first error
                  </label>
                </div>
              </div>
            </div>

            <div className="info-section">
              <h4>Timing</h4>
              <div className="form-grid three-cols">
                <div className="form-field">
                  <label>Vector Clock (MHz)</label>
                  <input
                    type="number"
                    min="1"
                    max="200"
                    value={testPlan.timing.vector_clock_mhz}
                    onChange={e => updateField('timing', 'vector_clock_mhz', parseInt(e.target.value) || 100)}
                  />
                </div>
                <div className="form-field">
                  <label>Setup Time (ns)</label>
                  <input
                    type="number"
                    min="0"
                    step="0.1"
                    value={testPlan.timing.setup_ns}
                    onChange={e => updateField('timing', 'setup_ns', parseFloat(e.target.value) || 0)}
                  />
                </div>
                <div className="form-field">
                  <label>Hold Time (ns)</label>
                  <input
                    type="number"
                    min="0"
                    step="0.1"
                    value={testPlan.timing.hold_ns}
                    onChange={e => updateField('timing', 'hold_ns', parseFloat(e.target.value) || 0)}
                  />
                </div>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'power' && (
          <div className="section-power">
            <div className="info-section">
              <h4>Power Sequence</h4>
              <div className="form-grid">
                <div className="form-field">
                  <label>Sequence Type</label>
                  <select
                    value={testPlan.power.sequence}
                    onChange={e => updateField('power', 'sequence', e.target.value)}
                  >
                    <option value="standard">Standard (rail order)</option>
                    <option value="custom">Custom</option>
                  </select>
                </div>
                <div className="form-field">
                  <label>Ramp Rate (mV/ms)</label>
                  <input
                    type="number"
                    min="1"
                    value={testPlan.power.ramp_rate_mv_per_ms}
                    onChange={e => updateField('power', 'ramp_rate_mv_per_ms', parseInt(e.target.value) || 10)}
                  />
                </div>
              </div>
            </div>

            <div className="info-section">
              <div className="section-header-row">
                <h4>Power Rails</h4>
                <button className="btn btn-small btn-primary" onClick={addRail}>+ Add Rail</button>
              </div>

              {testPlan.power.rails.length === 0 ? (
                <div className="empty-message">No power rails configured. Click "Add Rail" to add one.</div>
              ) : (
                <div className="rails-list">
                  {testPlan.power.rails.map((rail, idx) => (
                    <div key={idx} className="rail-item">
                      <div className="rail-header">
                        <span className="rail-id">Rail {rail.id}</span>
                        <button
                          className="btn btn-small btn-danger"
                          onClick={() => removeRail(idx)}
                        >Remove</button>
                      </div>
                      <div className="rail-fields">
                        <div className="form-field">
                          <label>Name</label>
                          <input
                            type="text"
                            value={rail.name}
                            onChange={e => updateRail(idx, 'name', e.target.value)}
                          />
                        </div>
                        <div className="form-field">
                          <label>Voltage (mV)</label>
                          <input
                            type="number"
                            min="0"
                            value={rail.voltage_mv}
                            onChange={e => updateRail(idx, 'voltage_mv', parseInt(e.target.value) || 0)}
                          />
                        </div>
                        <div className="form-field">
                          <label>Current Limit (mA)</label>
                          <input
                            type="number"
                            min="0"
                            value={rail.current_limit_ma}
                            onChange={e => updateRail(idx, 'current_limit_ma', parseInt(e.target.value) || 0)}
                          />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {activeSection === 'thermal' && (
          <div className="section-thermal">
            <div className="info-section">
              <h4>Thermal Settings</h4>
              <div className="form-grid three-cols">
                <div className="form-field">
                  <label>Setpoint (°C)</label>
                  <input
                    type="number"
                    min="-40"
                    max="150"
                    value={testPlan.thermal.setpoint_c}
                    onChange={e => updateField('thermal', 'setpoint_c', parseInt(e.target.value) || 25)}
                  />
                </div>
                <div className="form-field">
                  <label>Tolerance (°C)</label>
                  <input
                    type="number"
                    min="0"
                    max="20"
                    value={testPlan.thermal.tolerance_c}
                    onChange={e => updateField('thermal', 'tolerance_c', parseInt(e.target.value) || 2)}
                  />
                </div>
                <div className="form-field">
                  <label>Soak Time (s)</label>
                  <input
                    type="number"
                    min="0"
                    value={testPlan.thermal.soak_time_s}
                    onChange={e => updateField('thermal', 'soak_time_s', parseInt(e.target.value) || 0)}
                  />
                  <span className="field-hint">Wait time after reaching setpoint</span>
                </div>
              </div>

              {/* Visual temperature indicator */}
              <div className="thermal-visual">
                <div className="temp-bar">
                  <div
                    className="temp-range"
                    style={{
                      left: `${((testPlan.thermal.setpoint_c - testPlan.thermal.tolerance_c + 40) / 190) * 100}%`,
                      width: `${(testPlan.thermal.tolerance_c * 2 / 190) * 100}%`
                    }}
                  />
                  <div
                    className="temp-setpoint"
                    style={{
                      left: `${((testPlan.thermal.setpoint_c + 40) / 190) * 100}%`
                    }}
                  />
                </div>
                <div className="temp-labels">
                  <span>-40°C</span>
                  <span>25°C</span>
                  <span>85°C</span>
                  <span>150°C</span>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'preview' && (
          <div className="section-preview">
            <div className="preview-header">
              <h4>JSON Preview</h4>
              <button
                className="btn btn-small"
                onClick={() => navigator.clipboard.writeText(generateJson())}
              >Copy to Clipboard</button>
            </div>
            <pre className="preview-content">
              {generateJson()}
            </pre>
          </div>
        )}
      </div>

      {/* Info footer */}
      <div className="editor-footer">
        <p>
          Test plans are saved as JSON files. To run a test: load the plan in the main UI,
          select boards, and click Start. The GUI will send the appropriate commands to each board.
        </p>
      </div>
    </div>
  )
}
