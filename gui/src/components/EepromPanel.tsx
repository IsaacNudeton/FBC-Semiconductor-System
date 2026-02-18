import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './EepromPanel.css'

// BIM types matching firmware/src/hal/eeprom.rs
const BIM_TYPES = [
  { id: 0, name: 'Unknown' },
  { id: 1, name: 'Normandy' },
  { id: 2, name: 'Syros v2' },
  { id: 3, name: 'Aurora' },
  { id: 4, name: 'Iliad' },
] as const

// Backend response types
interface EepromResponse {
  raw: number[]
  header: EepromHeaderResponse
  rails: RailConfigResponse[]
  dut: DutMetadataResponse
  calibration: CalibrationResponse
}

interface EepromHeaderResponse {
  magic: number
  version: number
  bim_type: number
  hw_revision: number
  board_serial: number
  mfg_date: number
  asset_id: string
  config_crc: number
}

interface RailConfigResponse {
  rail_id: number
  name: number[]
  nominal_mv: number
  max_mv: number
  max_ma: number
  enabled_by_default: boolean
}

interface DutMetadataResponse {
  part_number: string
  lot_id: string
  wafer_id: number
  die_x: number
  die_y: number
  test_count: number
  last_test_time: number
}

interface CalibrationResponse {
  adc_offset: number[]
  adc_gain: number[]
  dac_offset: number[]
  dac_gain: number[]
  temp_offset: number
}

// UI display types
interface EepromData {
  magic: number
  version: number
  serial: number
  hw_revision: number
  bim_type: number
  asset_id: string
  vendor_id: string
  part_number: string
  rails: RailConfig[]
  dut_metadata: DutMetadata
  calibration: CalibrationData
  raw: number[]
}

interface RailConfig {
  index: number
  name: string
  nominal_mv: number
  min_mv: number
  max_mv: number
  enabled: boolean
}

interface DutMetadata {
  vendor: string
  part_number: string
  lot_id: string
  wafer_id: string
  test_date: string
}

interface CalibrationData {
  adc_offset: number[]
  adc_gain: number[]
  dac_offset: number[]
  temp_offset: number
  last_cal_date: string
}

export default function EepromPanel() {
  const { selectedBoard, connected } = useStore()
  const [eepromData, setEepromData] = useState<EepromData | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [activeSection, setActiveSection] = useState<'header' | 'rails' | 'dut' | 'cal' | 'raw'>('header')
  const [editMode, setEditMode] = useState(false)

  // Convert byte array to string (trim nulls)
  const bytesToString = (bytes: number[]): string => {
    return String.fromCharCode(...bytes.filter(b => b !== 0))
  }

  // Format Unix timestamp to date string
  const formatDate = (timestamp: number): string => {
    if (timestamp === 0) return 'N/A'
    return new Date(timestamp * 1000).toISOString().split('T')[0]
  }

  // Load EEPROM data
  useEffect(() => {
    if (!selectedBoard || !connected) {
      setEepromData(null)
      return
    }

    const fetchEeprom = async () => {
      setLoading(true)
      setError(null)
      try {
        const response = await invoke<EepromResponse>('read_eeprom', { mac: selectedBoard })

        // Convert backend response to UI format
        const data: EepromData = {
          magic: response.header.magic,
          version: response.header.version,
          serial: response.header.board_serial,
          hw_revision: response.header.hw_revision,
          bim_type: response.header.bim_type ?? 0,
          asset_id: response.header.asset_id ?? '',
          vendor_id: 'ISE Labs',
          part_number: `FBC-Z7020-R${response.header.hw_revision}`,
          rails: response.rails.map((r, i) => ({
            index: i,
            name: bytesToString(r.name) || `RAIL_${i}`,
            nominal_mv: r.nominal_mv,
            min_mv: Math.round(r.nominal_mv * 0.95),
            max_mv: r.max_mv,
            enabled: r.enabled_by_default,
          })),
          dut_metadata: {
            vendor: 'Unknown',
            part_number: response.dut.part_number,
            lot_id: response.dut.lot_id,
            wafer_id: `W${response.dut.wafer_id} (${response.dut.die_x},${response.dut.die_y})`,
            test_date: formatDate(response.dut.last_test_time),
          },
          calibration: {
            adc_offset: response.calibration.adc_offset,
            adc_gain: response.calibration.adc_gain.map(g => g / 1000), // Convert from fixed-point
            dac_offset: response.calibration.dac_offset,
            temp_offset: response.calibration.temp_offset / 10,
            last_cal_date: 'N/A',
          },
          raw: response.raw,
        }
        setEepromData(data)
      } catch (e) {
        console.error('Failed to read EEPROM:', e)
        setError(`Failed to read EEPROM: ${e}`)
      } finally {
        setLoading(false)
      }
    }

    fetchEeprom()
  }, [selectedBoard, connected])

  const handleSave = async () => {
    if (!eepromData || !selectedBoard) return

    try {
      await invoke('write_eeprom', { mac: selectedBoard, data: eepromData })
      setEditMode(false)
    } catch (e) {
      setError(`Failed to write EEPROM: ${e}`)
    }
  }

  if (!selectedBoard) {
    return (
      <div className="eeprom-panel">
        <div className="no-board-message">
          <span className="icon">💾</span>
          <h3>No Board Selected</h3>
          <p>Select a board to view EEPROM data.</p>
        </div>
      </div>
    )
  }

  if (loading) {
    return (
      <div className="eeprom-panel">
        <div className="loading">
          <div className="loading-spinner" />
          <p>Reading EEPROM...</p>
        </div>
      </div>
    )
  }

  if (!eepromData) {
    return (
      <div className="eeprom-panel">
        <div className="no-board-message">
          <span className="icon">⚠️</span>
          <h3>EEPROM Not Programmed</h3>
          <p>This board's EEPROM has not been initialized.</p>
          <button className="btn-primary" onClick={() => {}}>
            Initialize EEPROM
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className="eeprom-panel">
      {/* Header */}
      <div className="eeprom-header">
        <div className="header-info">
          <h2>EEPROM Configuration</h2>
          <div className="eeprom-summary">
            <span className="magic">Magic: 0x{eepromData.magic.toString(16).toUpperCase()}</span>
            <span className="version">v{eepromData.version}</span>
            <span className="serial">S/N: {eepromData.serial}</span>
          </div>
        </div>
        <div className="header-actions">
          <button className="btn-refresh" onClick={() => {}}>
            Refresh
          </button>
          {editMode ? (
            <>
              <button className="btn-save" onClick={handleSave}>Save</button>
              <button className="btn-cancel" onClick={() => setEditMode(false)}>Cancel</button>
            </>
          ) : (
            <button className="btn-edit" onClick={() => setEditMode(true)}>Edit</button>
          )}
        </div>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {/* Section Tabs */}
      <div className="section-tabs">
        <button
          className={activeSection === 'header' ? 'active' : ''}
          onClick={() => setActiveSection('header')}
        >
          Header
        </button>
        <button
          className={activeSection === 'rails' ? 'active' : ''}
          onClick={() => setActiveSection('rails')}
        >
          Rail Config
        </button>
        <button
          className={activeSection === 'dut' ? 'active' : ''}
          onClick={() => setActiveSection('dut')}
        >
          DUT Metadata
        </button>
        <button
          className={activeSection === 'cal' ? 'active' : ''}
          onClick={() => setActiveSection('cal')}
        >
          Calibration
        </button>
        <button
          className={activeSection === 'raw' ? 'active' : ''}
          onClick={() => setActiveSection('raw')}
        >
          Raw Hex
        </button>
      </div>

      {/* Content */}
      <div className="eeprom-content">
        {activeSection === 'header' && (
          <div className="section-content">
            <div className="info-grid">
              <div className="info-item">
                <label>Magic Number</label>
                <span className="mono">0x{eepromData.magic.toString(16).toUpperCase().padStart(8, '0')}</span>
              </div>
              <div className="info-item">
                <label>Version</label>
                <span>{eepromData.version}</span>
              </div>
              <div className="info-item">
                <label>BIM Type</label>
                {editMode ? (
                  <select
                    value={eepromData.bim_type}
                    onChange={(e) => setEepromData({...eepromData, bim_type: parseInt(e.target.value)})}
                    className="bim-type-select"
                  >
                    {BIM_TYPES.map(bt => (
                      <option key={bt.id} value={bt.id}>{bt.name}</option>
                    ))}
                  </select>
                ) : (
                  <span>{BIM_TYPES.find(bt => bt.id === eepromData.bim_type)?.name ?? 'Unknown'}</span>
                )}
              </div>
              <div className="info-item">
                <label>Asset ID (Board #)</label>
                {editMode ? (
                  <input
                    type="text"
                    value={eepromData.asset_id}
                    onChange={(e) => setEepromData({...eepromData, asset_id: e.target.value.slice(0, 6)})}
                    placeholder="BIM-042"
                    maxLength={6}
                    className="asset-id-input"
                  />
                ) : (
                  <span className="mono">{eepromData.asset_id || '(not set)'}</span>
                )}
              </div>
              <div className="info-item">
                <label>Serial Number</label>
                <span>{eepromData.serial}</span>
              </div>
              <div className="info-item">
                <label>HW Revision</label>
                <span>Rev {eepromData.hw_revision}</span>
              </div>
              <div className="info-item">
                <label>Vendor ID</label>
                <span>{eepromData.vendor_id}</span>
              </div>
              <div className="info-item full-width">
                <label>Part Number</label>
                <span>{eepromData.part_number}</span>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'rails' && (
          <div className="section-content">
            <div className="rail-table">
              <div className="rail-header">
                <span>Rail</span>
                <span>Name</span>
                <span>Nominal</span>
                <span>Min</span>
                <span>Max</span>
                <span>Enabled</span>
              </div>
              {eepromData.rails.map(rail => (
                <div key={rail.index} className={`rail-row ${rail.enabled ? '' : 'disabled'}`}>
                  <span className="rail-index">{rail.index}</span>
                  <span className="rail-name">{rail.name}</span>
                  <span className="rail-nominal">{rail.nominal_mv} mV</span>
                  <span className="rail-min">{rail.min_mv} mV</span>
                  <span className="rail-max">{rail.max_mv} mV</span>
                  <span className={`rail-enabled ${rail.enabled ? 'yes' : 'no'}`}>
                    {rail.enabled ? 'Yes' : 'No'}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {activeSection === 'dut' && (
          <div className="section-content">
            <div className="dut-info">
              <div className="dut-item">
                <label>DUT Vendor</label>
                <span>{eepromData.dut_metadata.vendor}</span>
              </div>
              <div className="dut-item">
                <label>Part Number</label>
                <span>{eepromData.dut_metadata.part_number}</span>
              </div>
              <div className="dut-item">
                <label>Lot ID</label>
                <span className="mono">{eepromData.dut_metadata.lot_id}</span>
              </div>
              <div className="dut-item">
                <label>Wafer ID</label>
                <span className="mono">{eepromData.dut_metadata.wafer_id}</span>
              </div>
              <div className="dut-item">
                <label>Test Date</label>
                <span>{eepromData.dut_metadata.test_date}</span>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'cal' && (
          <div className="section-content">
            <div className="cal-section">
              <h4>ADC Calibration</h4>
              <div className="cal-table">
                <div className="cal-header">
                  <span>CH</span>
                  <span>Offset</span>
                  <span>Gain</span>
                </div>
                {eepromData.calibration.adc_offset.map((offset, i) => (
                  <div key={i} className="cal-row">
                    <span className="cal-ch">{i}</span>
                    <span className={`cal-offset ${offset !== 0 ? 'adjusted' : ''}`}>
                      {offset >= 0 ? '+' : ''}{offset}
                    </span>
                    <span className={`cal-gain ${eepromData.calibration.adc_gain[i] !== 1.0 ? 'adjusted' : ''}`}>
                      {eepromData.calibration.adc_gain[i].toFixed(3)}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            <div className="cal-section">
              <h4>DAC Calibration</h4>
              <div className="cal-table compact">
                <div className="cal-header">
                  <span>CH</span>
                  <span>Offset</span>
                </div>
                {eepromData.calibration.dac_offset.map((offset, i) => (
                  <div key={i} className="cal-row">
                    <span className="cal-ch">{i}</span>
                    <span className={`cal-offset ${offset !== 0 ? 'adjusted' : ''}`}>
                      {offset >= 0 ? '+' : ''}{offset}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            <div className="cal-section">
              <h4>Temperature</h4>
              <div className="cal-item">
                <label>Offset</label>
                <span className={eepromData.calibration.temp_offset !== 0 ? 'adjusted' : ''}>
                  {eepromData.calibration.temp_offset >= 0 ? '+' : ''}{eepromData.calibration.temp_offset}°C
                </span>
              </div>
              <div className="cal-item">
                <label>Last Calibration</label>
                <span>{eepromData.calibration.last_cal_date}</span>
              </div>
            </div>
          </div>
        )}

        {activeSection === 'raw' && (
          <div className="section-content">
            <div className="hex-viewer">
              <div className="hex-header">
                <span className="offset-col">Offset</span>
                {Array.from({ length: 16 }).map((_, i) => (
                  <span key={i} className="hex-col">{i.toString(16).toUpperCase()}</span>
                ))}
                <span className="ascii-col">ASCII</span>
              </div>
              {Array.from({ length: 16 }).map((_, row) => {
                const offset = row * 16
                const bytes = eepromData.raw.slice(offset, offset + 16)
                return (
                  <div key={row} className="hex-row">
                    <span className="offset-col">{offset.toString(16).toUpperCase().padStart(2, '0')}</span>
                    {bytes.map((byte, i) => (
                      <span key={i} className="hex-col">{byte.toString(16).toUpperCase().padStart(2, '0')}</span>
                    ))}
                    <span className="ascii-col">
                      {bytes.map(b => (b >= 32 && b <= 126 ? String.fromCharCode(b) : '.')).join('')}
                    </span>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
