import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import { useHardwareCommand } from '../hooks/useHardwareCommand'
import './PowerControlPanel.css'

interface VicorCoreResponse {
  id: number
  enabled: boolean
  voltage_mv: number
  current_ma: number
  temp_c: number
  status: string
}

interface VicorStatusResponse {
  cores: VicorCoreResponse[]
}

interface PmBusRailResponse {
  address: number
  name: string
  enabled: boolean
  voltage_mv: number
  current_ma: number
  power_mw: number
  temp_c: number
  status_word: number
}

interface PmBusStatusResponse {
  rails: PmBusRailResponse[]
}

interface VicorCore {
  id: number
  enabled: boolean
  voltage_mv: number
  target_mv: number
  current_ma: number
  temp_c: number
}

interface PmBusRail {
  id: number
  name: string
  address: number
  enabled: boolean
  voltage_mv: number
  current_ma: number
  temperature_c: number
  status: 'ok' | 'warning' | 'fault'
}

// Sonoma ADC reading used to populate VICOR card voltages
interface SonomaAnalogReading {
  channel: number
  raw: number
  voltage_mv: number
}

export default function PowerControlPanel() {
  const { selectedBoard, connected, controlMode, selectedSonomaBoard } = useStore()
  const { exec, hasBoard } = useHardwareCommand()
  const [vicorCores, setVicorCores] = useState<VicorCore[]>([])
  const [pmBusRails, setPmBusRails] = useState<PmBusRail[]>([])
  const [activeTab, setActiveTab] = useState<'vicor' | 'pmbus'>('vicor')
  const [editingCore, setEditingCore] = useState<number | null>(null)
  const [editVoltage, setEditVoltage] = useState('')

  useEffect(() => {
    if (!hasBoard) {
      setVicorCores([])
      setPmBusRails([])
      return
    }

    const fetchPowerStatus = async () => {
      try {
        if (controlMode === 'fbc') {
          // FBC mode: use existing protocol
          const vicorResult = await invoke<VicorStatusResponse>('get_vicor_status', { mac: selectedBoard })
          setVicorCores(vicorResult.cores.map(c => ({
            id: c.id,
            enabled: c.enabled,
            voltage_mv: c.voltage_mv,
            target_mv: c.voltage_mv,
            current_ma: c.current_ma,
            temp_c: c.temp_c,
          })))

          const pmBusResult = await invoke<PmBusStatusResponse>('get_pmbus_status', { mac: selectedBoard })
          setPmBusRails(pmBusResult.rails.map((r, i) => ({
            id: i + 1,
            name: r.name,
            address: r.address,
            enabled: r.enabled,
            voltage_mv: r.voltage_mv,
            current_ma: r.current_ma,
            temperature_c: r.temp_c,
            status: r.status_word === 0 ? 'ok' : r.status_word & 0xFF00 ? 'fault' : 'warning',
          })))
        } else {
          // Sonoma mode: read ADC32 to get VICOR core voltages
          const adc = await invoke<SonomaAnalogReading[]>('sonoma_read_adc32', { ip: selectedSonomaBoard })
          // Channels 0-5 map to VDD_CORE1-6 on external ADC
          setVicorCores([1, 2, 3, 4, 5, 6].map(id => {
            const reading = adc.find(r => r.channel === id - 1)
            const voltage_mv = reading ? reading.voltage_mv : 0
            return {
              id,
              enabled: voltage_mv > 50,
              voltage_mv,
              target_mv: voltage_mv,
              current_ma: 0, // Sonoma doesn't report per-core current via ADC
              temp_c: 0,
            }
          }))
          // PMBus not readable via Sonoma SSH — leave empty
          setPmBusRails([])
        }
      } catch (e) {
        console.error('Failed to fetch power status:', e)
      }
    }

    fetchPowerStatus()
    const interval = setInterval(fetchPowerStatus, 2000)
    return () => clearInterval(interval)
  }, [selectedBoard, connected, controlMode, selectedSonomaBoard, hasBoard])

  const handleCoreToggle = async (coreId: number) => {
    const core = vicorCores.find(c => c.id === coreId)
    if (!core) return

    try {
      if (controlMode === 'fbc') {
        await invoke('set_vicor_enable', {
          mac: selectedBoard,
          coreId: coreId,
          enable: !core.enabled
        })
      } else {
        if (core.enabled) {
          await invoke('sonoma_vicor_disable', { ip: selectedSonomaBoard, core: coreId })
        } else {
          // Init with a default voltage (840mV)
          await invoke('sonoma_vicor_init', { ip: selectedSonomaBoard, core: coreId, voltage: 0.84 })
        }
      }
      setVicorCores(prev => prev.map(c =>
        c.id === coreId ? { ...c, enabled: !c.enabled } : c
      ))
    } catch (e) {
      console.error('Failed to toggle core:', e)
    }
  }

  const handleSetVoltage = async (coreId: number) => {
    const voltage = parseInt(editVoltage)
    if (isNaN(voltage) || voltage < 500 || voltage > 1500) {
      alert('Voltage must be between 500-1500 mV')
      return
    }

    try {
      if (controlMode === 'fbc') {
        await invoke('set_vicor_voltage', {
          mac: selectedBoard,
          coreId: coreId,
          voltageMv: voltage
        })
      } else {
        // Sonoma expects volts, not millivolts
        await invoke('sonoma_vicor_voltage', {
          ip: selectedSonomaBoard,
          core: coreId,
          voltage: voltage / 1000.0,
        })
      }
      setVicorCores(prev => prev.map(c =>
        c.id === coreId ? { ...c, target_mv: voltage } : c
      ))
      setEditingCore(null)
    } catch (e) {
      console.error('Failed to set voltage:', e)
    }
  }

  const handlePmBusToggle = async (address: number, currentEnabled: boolean) => {
    try {
      if (controlMode === 'fbc') {
        await invoke('set_pmbus_enable', {
          mac: selectedBoard,
          address: address,
          enable: !currentEnabled
        })
      } else {
        if (currentEnabled) {
          await invoke('sonoma_pmbus_off', { ip: selectedSonomaBoard, channel: address })
        } else {
          await invoke('sonoma_pmbus_set', { ip: selectedSonomaBoard, channel: address, voltage: 1.0 })
        }
      }
      setPmBusRails(prev => prev.map(r =>
        r.address === address ? { ...r, enabled: !r.enabled } : r
      ))
    } catch (e) {
      console.error('Failed to toggle PMBus rail:', e)
    }
  }

  const handleEmergencyStop = async () => {
    if (!confirm('This will disable ALL power supplies immediately. Continue?')) return

    try {
      if (controlMode === 'fbc') {
        await invoke('emergency_stop', { mac: selectedBoard })
      } else {
        await invoke('sonoma_emergency_stop', { ip: selectedSonomaBoard })
      }
      setVicorCores(prev => prev.map(c => ({ ...c, enabled: false, voltage_mv: 0 })))
      setPmBusRails(prev => prev.map(r => ({ ...r, enabled: false, voltage_mv: 0 })))
    } catch (e) {
      console.error('Emergency stop failed:', e)
    }
  }

  const handlePowerSequence = async (action: 'on' | 'off') => {
    try {
      if (controlMode === 'fbc') {
        await invoke(action === 'on' ? 'power_sequence_on' : 'power_sequence_off', {
          mac: selectedBoard
        })
      } else {
        if (action === 'off') {
          await invoke('sonoma_emergency_stop', { ip: selectedSonomaBoard })
        } else {
          // Sonoma init sequence
          await invoke('sonoma_init', { ip: selectedSonomaBoard })
        }
      }
    } catch (e) {
      console.error('Power sequence failed:', e)
    }
  }

  const totalPower = (
    vicorCores.reduce((sum, c) => sum + (c.enabled ? c.voltage_mv * c.current_ma / 1000000 : 0), 0) +
    pmBusRails.reduce((sum, r) => sum + (r.enabled ? r.voltage_mv * r.current_ma / 1000000 : 0), 0)
  )

  if (!hasBoard) {
    return (
      <div className="power-control-panel">
        <div className="no-board-message">
          <span className="icon">⚡</span>
          <h3>No Board Selected</h3>
          <p>Select a board to control power supplies.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="power-control-panel">
      {/* Header with Emergency Stop */}
      <div className="power-header">
        <div className="header-info">
          <h2>Power Control</h2>
          <div className="power-summary">
            <span className="power-total">Total: {totalPower.toFixed(1)}W</span>
            {controlMode === 'sonoma' && (
              <span className="power-mode-badge">SSH</span>
            )}
          </div>
        </div>
        <div className="header-actions">
          <button className="btn btn-sequence" onClick={() => handlePowerSequence('on')}>
            ⚡ {controlMode === 'sonoma' ? 'Init' : 'Power On'}
          </button>
          <button className="btn btn-sequence off" onClick={() => handlePowerSequence('off')}>
            Power Off
          </button>
          <button className="btn-emergency" onClick={handleEmergencyStop}>
            🚨 EMERGENCY STOP
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="power-tabs">
        <button
          className={activeTab === 'vicor' ? 'active' : ''}
          onClick={() => setActiveTab('vicor')}
        >
          VICOR Cores ({vicorCores.filter(c => c.enabled).length}/{vicorCores.length})
        </button>
        <button
          className={activeTab === 'pmbus' ? 'active' : ''}
          onClick={() => setActiveTab('pmbus')}
        >
          PMBus Rails ({pmBusRails.filter(r => r.enabled).length}/{pmBusRails.length})
        </button>
      </div>

      {/* Content */}
      <div className="power-content">
        {activeTab === 'vicor' && (
          <div className="vicor-grid">
            {vicorCores.map(core => (
              <div key={core.id} className={`core-card ${core.enabled ? 'enabled' : 'disabled'}`}>
                <div className="core-header">
                  <span className="core-label">Core {core.id}</span>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={core.enabled}
                      onChange={() => handleCoreToggle(core.id)}
                    />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div className="core-voltage">
                  {editingCore === core.id ? (
                    <div className="voltage-edit">
                      <input
                        type="number"
                        value={editVoltage}
                        onChange={e => setEditVoltage(e.target.value)}
                        placeholder="mV"
                        min={500}
                        max={1500}
                        autoFocus
                      />
                      <button onClick={() => handleSetVoltage(core.id)}>✓</button>
                      <button onClick={() => setEditingCore(null)}>✗</button>
                    </div>
                  ) : (
                    <div
                      className="voltage-display"
                      onClick={() => {
                        setEditingCore(core.id)
                        setEditVoltage(core.target_mv.toString())
                      }}
                    >
                      <span className="value">{core.enabled ? core.voltage_mv : '—'}</span>
                      <span className="unit">mV</span>
                      {core.enabled && <span className="edit-hint">✎</span>}
                    </div>
                  )}
                  <div className="target-voltage">
                    Target: {core.target_mv} mV
                  </div>
                </div>

                <div className="core-current">
                  <div className="current-bar">
                    <div
                      className="current-fill"
                      style={{ width: `${Math.min(100, (core.current_ma / 50000) * 100)}%` }}
                    />
                  </div>
                  <span className="current-value">
                    {core.enabled ? (core.current_ma / 1000).toFixed(1) : '—'} A
                  </span>
                </div>

                <div className="core-power">
                  {core.enabled
                    ? `${((core.voltage_mv * core.current_ma) / 1000000).toFixed(1)}W`
                    : '—'
                  }
                </div>
              </div>
            ))}
          </div>
        )}

        {activeTab === 'pmbus' && (
          <div className="pmbus-table">
            {controlMode === 'sonoma' && pmBusRails.length === 0 ? (
              <div className="no-board-message" style={{ padding: '24px' }}>
                <p>PMBus readback not available via SSH. Use individual set/off commands.</p>
              </div>
            ) : (
              <>
                <div className="table-header">
                  <span>Rail</span>
                  <span>Address</span>
                  <span>Voltage</span>
                  <span>Current</span>
                  <span>Temp</span>
                  <span>Status</span>
                  <span>Enable</span>
                </div>
                {pmBusRails.map(rail => (
                  <div key={rail.id} className={`table-row ${rail.enabled ? '' : 'disabled'}`}>
                    <span className="rail-name">{rail.name}</span>
                    <span className="rail-address mono">0x{rail.address.toString(16).padStart(2, '0')}</span>
                    <span className="rail-voltage">{rail.enabled ? `${rail.voltage_mv} mV` : '—'}</span>
                    <span className="rail-current">{rail.enabled ? `${rail.current_ma} mA` : '—'}</span>
                    <span className="rail-temp">{rail.temperature_c}°C</span>
                    <span className={`rail-status status-${rail.status}`}>
                      {rail.status === 'ok' ? '●' : rail.status === 'warning' ? '◐' : '○'}
                    </span>
                    <label className="toggle-switch small">
                      <input
                        type="checkbox"
                        checked={rail.enabled}
                        onChange={() => handlePmBusToggle(rail.address, rail.enabled)}
                      />
                      <span className="toggle-slider" />
                    </label>
                  </div>
                ))}
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
