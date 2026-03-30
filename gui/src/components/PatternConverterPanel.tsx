import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import './PatternConverterPanel.css'

type Tab = 'convert' | 'device-config' | 'pin-import'

interface ConvertResult {
  success: boolean
  num_signals: number
  num_vectors: number
  hex_path: string | null
  seq_path: string | null
  fbc_path: string | null
  version: string
}

interface DcResult {
  success: boolean
  profile: string
  num_channels: number
  num_supplies: number
  num_steps: number
  output_dir: string
  device_name?: string
}

interface ExtractedChannel {
  signal_name: string
  channel: number
  direction: string
  voltage: number | null
  group: string | null
}

interface ExtractedSupply {
  core_name: string
  voltage: number
  sequence_order: number
  ramp_delay_ms: number
}

interface BankVoltage {
  bank_name: string
  voltage: number
}

interface ExtractedPinTable {
  device_name: string
  source_format: string
  channels: ExtractedChannel[]
  supplies: ExtractedSupply[]
  bank_voltages: BankVoltage[]
  warnings: string[]
}

interface PinMismatch {
  signal_name: string
  field: string
  primary_value: string
  secondary_value: string
}

interface VerificationResult {
  primary: ExtractedPinTable
  mismatches: PinMismatch[]
  match_count: number
  mismatch_count: number
}

export default function PatternConverterPanel() {
  const [tab, setTab] = useState<Tab>('convert')

  // ── Convert state ──
  const [inputPath, setInputPath] = useState('')
  const [pinmapPath, setPinmapPath] = useState('')
  const [hexOutput, setHexOutput] = useState('')
  const [seqOutput, setSeqOutput] = useState('')
  const [fbcOutput, setFbcOutput] = useState('')
  const [format, setFormat] = useState('auto')
  const [converting, setConverting] = useState(false)
  const [convertResult, setConvertResult] = useState<ConvertResult | null>(null)

  // ── Device Config state ──
  const [profile, setProfile] = useState('sonoma')
  const [devicePath, setDevicePath] = useState('')
  const [outputDir, setOutputDir] = useState('')
  const [generating, setGenerating] = useState(false)
  const [dcResult, setDcResult] = useState<DcResult | null>(null)

  // ── Pin Import state ──
  const [importPath, setImportPath] = useState('')
  const [secondaryPath, setSecondaryPath] = useState('')
  const [importProfile, setImportProfile] = useState('sonoma')
  const [importOutputDir, setImportOutputDir] = useState('')
  const [extracting, setExtracting] = useState(false)
  const [importGenerating, setImportGenerating] = useState(false)
  const [pinTable, setPinTable] = useState<ExtractedPinTable | null>(null)
  const [verification, setVerification] = useState<VerificationResult | null>(null)
  const [importResult, setImportResult] = useState<DcResult | null>(null)

  // ── Shared ──
  const [error, setError] = useState('')

  // ── File pickers ──
  const browseInput = async () => {
    const path = await open({
      title: 'Select Pattern File',
      filters: [
        { name: 'Pattern Files', extensions: ['atp', 'stil', 'avc'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (path) setInputPath(path as string)
  }

  const browsePinmap = async () => {
    const path = await open({
      title: 'Select Pin Map',
      filters: [
        { name: 'Pin Map', extensions: ['txt', 'cfg', 'pinmap'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (path) setPinmapPath(path as string)
  }

  const browseHexOutput = async () => {
    const path = await open({
      title: 'Save .hex Output',
      filters: [{ name: 'Hex Binary', extensions: ['hex'] }],
    })
    if (path) setHexOutput(path as string)
  }

  const browseSeqOutput = async () => {
    const path = await open({
      title: 'Save .seq Output',
      filters: [{ name: 'Sequence File', extensions: ['seq'] }],
    })
    if (path) setSeqOutput(path as string)
  }

  const browseFbcOutput = async () => {
    const path = await open({
      title: 'Save .fbc Output (compressed FBC)',
      filters: [{ name: 'FBC Compressed', extensions: ['fbc'] }],
    })
    if (path) setFbcOutput(path as string)
  }

  const browseDevice = async () => {
    const path = await open({
      title: 'Select Device Config',
      filters: [
        { name: 'JSON Config', extensions: ['json'] },
        { name: 'CSV Config', extensions: ['csv'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (path) setDevicePath(path as string)
  }

  const browseOutputDir = async () => {
    const path = await open({ title: 'Select Output Directory', directory: true })
    if (path) setOutputDir(path as string)
  }

  const browseImportFile = async () => {
    const path = await open({
      title: 'Select Pin Table Source',
      filters: [
        { name: 'Pin Tables', extensions: ['csv', 'xlsx', 'xls', 'pdf'] },
        { name: 'CSV', extensions: ['csv', 'tsv', 'txt'] },
        { name: 'Excel', extensions: ['xlsx', 'xls', 'xlsm', 'ods'] },
        { name: 'PDF', extensions: ['pdf'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (path) {
      setImportPath(path as string)
      setPinTable(null)
      setVerification(null)
      setImportResult(null)
    }
  }

  const browseSecondaryFile = async () => {
    const path = await open({
      title: 'Select Secondary Source (for verification)',
      filters: [
        { name: 'Pin Tables', extensions: ['csv', 'xlsx', 'xls', 'pdf'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (path) {
      setSecondaryPath(path as string)
      setVerification(null)
    }
  }

  const browseImportOutputDir = async () => {
    const path = await open({ title: 'Select Output Directory', directory: true })
    if (path) setImportOutputDir(path as string)
  }

  // ── Convert ──
  const handleConvert = async () => {
    if (!inputPath) {
      setError('Select a pattern file first')
      return
    }
    if (!hexOutput && !seqOutput && !fbcOutput) {
      setError('Specify at least one output path (.hex, .seq, or .fbc)')
      return
    }

    setConverting(true)
    setError('')
    setConvertResult(null)

    try {
      const result = await invoke<ConvertResult>('pc_convert', {
        inputPath,
        pinmapPath: pinmapPath || null,
        hexOutput: hexOutput || null,
        seqOutput: seqOutput || null,
        fbcOutput: fbcOutput || null,
        format: format === 'auto' ? null : format,
      })
      setConvertResult(result)
    } catch (e) {
      setError(String(e))
    } finally {
      setConverting(false)
    }
  }

  // ── Generate Device Config ──
  const handleGenerate = async () => {
    if (!devicePath) {
      setError('Select a device config file first')
      return
    }
    if (!outputDir) {
      setError('Select an output directory')
      return
    }

    setGenerating(true)
    setError('')
    setDcResult(null)

    try {
      const result = await invoke<DcResult>('dc_generate_config', {
        profile,
        devicePath,
        outputDir,
      })
      setDcResult(result)
    } catch (e) {
      setError(String(e))
    } finally {
      setGenerating(false)
    }
  }

  // ── Extract Pin Table ──
  const handleExtract = async () => {
    if (!importPath) {
      setError('Select a source file first')
      return
    }

    setExtracting(true)
    setError('')
    setPinTable(null)
    setVerification(null)
    setImportResult(null)

    try {
      if (secondaryPath) {
        // Cross-verify mode
        const result = await invoke<VerificationResult>('verify_pin_tables', {
          primaryPath: importPath,
          secondaryPath,
        })
        setPinTable(result.primary)
        setVerification(result)
      } else {
        // Single source mode
        const result = await invoke<ExtractedPinTable>('extract_pin_table', {
          filePath: importPath,
        })
        setPinTable(result)
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setExtracting(false)
    }
  }

  // ── Generate from extracted ──
  const handleGenerateFromExtracted = async () => {
    if (!pinTable) {
      setError('Extract pin data first')
      return
    }
    if (!importOutputDir) {
      setError('Select an output directory')
      return
    }

    setImportGenerating(true)
    setError('')
    setImportResult(null)

    try {
      const result = await invoke<DcResult>('generate_from_extracted', {
        data: pinTable,
        profile: importProfile,
        outputDir: importOutputDir,
      })
      setImportResult(result)
    } catch (e) {
      setError(String(e))
    } finally {
      setImportGenerating(false)
    }
  }

  // ── Inline editing helpers ──
  const updateChannel = (idx: number, field: keyof ExtractedChannel, value: string) => {
    if (!pinTable) return
    const updated = { ...pinTable }
    const ch = { ...updated.channels[idx] }
    if (field === 'channel') {
      ch.channel = parseInt(value) || 0
    } else if (field === 'voltage') {
      ch.voltage = value === '' ? null : parseFloat(value) || null
    } else {
      (ch as any)[field] = value
    }
    updated.channels = [...updated.channels]
    updated.channels[idx] = ch
    setPinTable(updated)
  }

  const addChannel = () => {
    if (!pinTable) return
    const maxCh = pinTable.channels.reduce((m, c) => Math.max(m, c.channel), -1)
    setPinTable({
      ...pinTable,
      channels: [...pinTable.channels, {
        signal_name: '',
        channel: maxCh + 1,
        direction: 'IO',
        voltage: null,
        group: null,
      }],
    })
  }

  const deleteChannel = (idx: number) => {
    if (!pinTable) return
    setPinTable({
      ...pinTable,
      channels: pinTable.channels.filter((_, i) => i !== idx),
    })
  }

  const updateSupply = (idx: number, field: keyof ExtractedSupply, value: string) => {
    if (!pinTable) return
    const updated = { ...pinTable }
    const sup = { ...updated.supplies[idx] }
    if (field === 'voltage' || field === 'ramp_delay_ms') {
      (sup as any)[field] = parseFloat(value) || 0
    } else if (field === 'sequence_order') {
      sup.sequence_order = parseInt(value) || 0
    } else {
      (sup as any)[field] = value
    }
    updated.supplies = [...updated.supplies]
    updated.supplies[idx] = sup
    setPinTable(updated)
  }

  const addSupply = () => {
    if (!pinTable) return
    setPinTable({
      ...pinTable,
      supplies: [...pinTable.supplies, {
        core_name: `CORE${pinTable.supplies.length + 1}`,
        voltage: 0,
        sequence_order: pinTable.supplies.length,
        ramp_delay_ms: 10,
      }],
    })
  }

  const deleteSupply = (idx: number) => {
    if (!pinTable) return
    setPinTable({
      ...pinTable,
      supplies: pinTable.supplies.filter((_, i) => i !== idx),
    })
  }

  // Check if a channel field has a mismatch
  const getMismatch = (signalName: string, field: string): PinMismatch | undefined => {
    if (!verification) return undefined
    return verification.mismatches.find(
      m => m.signal_name.toLowerCase() === signalName.toLowerCase() && m.field === field
    )
  }

  const getFormatBadge = (path: string) => {
    const ext = path.split('.').pop()?.toLowerCase() || ''
    if (['csv', 'tsv', 'txt'].includes(ext)) return 'csv'
    if (['xlsx', 'xls', 'xlsm', 'ods'].includes(ext)) return 'xlsx'
    if (ext === 'pdf') return 'pdf'
    return ''
  }

  return (
    <div className="pc-panel">
      {/* Header */}
      <div className="pc-header">
        <div className="header-info">
          <h2>Pattern Converter</h2>
          <div className="config-summary">
            <span className="device-name">C Engine v{convertResult?.version || '1.0.0'}</span>
            <span className="device-type">ATP / STIL / AVC</span>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="config-tabs">
        <button className={tab === 'convert' ? 'active' : ''} onClick={() => setTab('convert')}>
          Pattern Conversion
        </button>
        <button className={tab === 'device-config' ? 'active' : ''} onClick={() => setTab('device-config')}>
          Device Config
        </button>
        <button className={tab === 'pin-import' ? 'active' : ''} onClick={() => setTab('pin-import')}>
          Pin Import
        </button>
      </div>

      {/* Error */}
      {error && <div className="error-banner">{error}</div>}

      {/* Content */}
      <div className="config-content">
        {tab === 'convert' && (
          <div className="pc-convert-content">
            <h3>Convert Pattern to Binary</h3>
            <p className="profile-description">
              ATP/STIL/AVC + PIN_MAP &rarr; .hex (legacy 40B/vec) + .seq + .fbc (compressed FBC)
            </p>

            {/* File inputs */}
            <div className="profile-files">
              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">INPUT</span>
                  <span className="file-desc">Pattern file (.atp, .stil, .avc)</span>
                </div>
                <div className="file-value">
                  {inputPath ? (
                    <span className="file-path">{inputPath}</span>
                  ) : (
                    <span className="file-empty">No file selected</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseInput}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">PIN MAP</span>
                  <span className="file-desc">Optional channel mapping</span>
                </div>
                <div className="file-value">
                  {pinmapPath ? (
                    <span className="file-path">{pinmapPath}</span>
                  ) : (
                    <span className="file-empty">Identity map (signal N = channel N)</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browsePinmap}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.hex OUT</span>
                  <span className="file-desc">Binary vector output</span>
                </div>
                <div className="file-value">
                  {hexOutput ? (
                    <span className="file-path">{hexOutput}</span>
                  ) : (
                    <span className="file-empty">Not set</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseHexOutput}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.seq OUT</span>
                  <span className="file-desc">Sequence file output</span>
                </div>
                <div className="file-value">
                  {seqOutput ? (
                    <span className="file-path">{seqOutput}</span>
                  ) : (
                    <span className="file-empty">Not set</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseSeqOutput}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">.fbc OUT</span>
                  <span className="file-desc">Compressed FBC binary (4-710x smaller)</span>
                </div>
                <div className="file-value">
                  {fbcOutput ? (
                    <span className="file-path">{fbcOutput}</span>
                  ) : (
                    <span className="file-empty">Not set</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseFbcOutput}>Browse</button>
              </div>
            </div>

            {/* Format selector */}
            <div className="pc-format-row">
              <label>Format:</label>
              <select value={format} onChange={(e) => setFormat(e.target.value)}>
                <option value="auto">Auto-detect</option>
                <option value="atp">ATP</option>
                <option value="stil">STIL</option>
                <option value="avc">AVC</option>
              </select>
            </div>

            {/* Actions */}
            <div className="profile-actions">
              <button
                className="btn-compile"
                onClick={handleConvert}
                disabled={converting || !inputPath}
              >
                {converting ? 'Converting...' : 'Convert'}
              </button>
              <button className="btn-clear" onClick={() => {
                setInputPath('')
                setPinmapPath('')
                setHexOutput('')
                setSeqOutput('')
                setFbcOutput('')
                setConvertResult(null)
                setError('')
              }}>
                Clear
              </button>
            </div>

            {/* Result */}
            {convertResult && (
              <div className="pc-result">
                <div className="info-cards">
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Signals</span>
                      <span className="card-value">{convertResult.num_signals}</span>
                    </div>
                  </div>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Vectors</span>
                      <span className="card-value">{convertResult.num_vectors}</span>
                    </div>
                  </div>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Hex Size</span>
                      <span className="card-value">{(convertResult.num_vectors * 40 / 1024).toFixed(1)} KB</span>
                    </div>
                  </div>
                  {convertResult.fbc_path && (
                    <div className="info-card">
                      <div className="card-content">
                        <span className="card-label">.fbc</span>
                        <span className="card-value pc-success">Compressed</span>
                      </div>
                    </div>
                  )}
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Status</span>
                      <span className="card-value pc-success">Done</span>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {tab === 'device-config' && (
          <div className="pc-dc-content">
            <h3>Generate Device Config Files</h3>
            <p className="profile-description">
              Device JSON + Tester Profile &rarr; PIN_MAP + .map + .lvl + .tim + .tp + PowerOn/Off
            </p>

            {/* Profile selector */}
            <div className="pc-format-row">
              <label>Tester Profile:</label>
              <select value={profile} onChange={(e) => setProfile(e.target.value)}>
                <option value="sonoma">Sonoma — 128ch, 6 cores, Zynq 7020</option>
                <option value="hx">HX — 160ch/axis × 4 axes, 16 supplies, Incal</option>
                <option value="xp160">XP-160/Shasta — 160ch/axis × 8 axes, 32 supplies, Incal</option>
                <option value="mcc">MCC — 128ch, 8 supplies, ISE Labs</option>
              </select>
              <span className="pc-hint">or browse for custom profile JSON</span>
            </div>

            {/* File inputs */}
            <div className="profile-files">
              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">DEVICE</span>
                  <span className="file-desc">Device config (.json or .csv)</span>
                </div>
                <div className="file-value">
                  {devicePath ? (
                    <span className="file-path">{devicePath}</span>
                  ) : (
                    <span className="file-empty">No file selected</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseDevice}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">OUTPUT</span>
                  <span className="file-desc">Directory for generated files</span>
                </div>
                <div className="file-value">
                  {outputDir ? (
                    <span className="file-path">{outputDir}</span>
                  ) : (
                    <span className="file-empty">Select output directory</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseOutputDir}>Browse</button>
              </div>
            </div>

            {/* Actions */}
            <div className="profile-actions">
              <button
                className="btn-compile"
                onClick={handleGenerate}
                disabled={generating || !devicePath || !outputDir}
              >
                {generating ? 'Generating...' : 'Generate All'}
              </button>
              <button className="btn-clear" onClick={() => {
                setDevicePath('')
                setOutputDir('')
                setDcResult(null)
                setError('')
              }}>
                Clear
              </button>
            </div>

            {/* Result */}
            {dcResult && (
              <div className="pc-result">
                <div className="profile-summary">
                  <span className="summary-value">Generated successfully</span>
                </div>
                <div className="info-cards" style={{ marginTop: 16 }}>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Profile</span>
                      <span className="card-value">{dcResult.profile}</span>
                    </div>
                  </div>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Channels</span>
                      <span className="card-value">{dcResult.num_channels}</span>
                    </div>
                  </div>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Supplies</span>
                      <span className="card-value">{dcResult.num_supplies}</span>
                    </div>
                  </div>
                  <div className="info-card">
                    <div className="card-content">
                      <span className="card-label">Test Steps</span>
                      <span className="card-value">{dcResult.num_steps}</span>
                    </div>
                  </div>
                </div>
                <div className="pc-output-files">
                  <h4>Generated Files:</h4>
                  <ul>
                    <li><code>PIN_MAP</code> — channel mapping for pattern converter</li>
                    <li><code>.map</code> — signal to GPIO mapping</li>
                    <li><code>.lvl</code> — bank voltages + VIH/VIL/VOH/VOL</li>
                    <li><code>.tim</code> — period + drive/compare edges</li>
                    <li><code>.tp</code> — test plan steps</li>
                    <li><code>PowerOn.sh</code> — power-on sequence script</li>
                    <li><code>PowerOff.sh</code> — power-off sequence script</li>
                  </ul>
                </div>
              </div>
            )}
          </div>
        )}

        {tab === 'pin-import' && (
          <div className="pc-import-content">
            <h3>Import Pin Table</h3>
            <p className="profile-description">
              CSV/Excel/PDF &rarr; editable pin table &rarr; device config files
            </p>

            {/* Source files */}
            <div className="profile-files">
              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">SOURCE</span>
                  <span className="file-desc">Pin table (.csv, .xlsx, .pdf)</span>
                </div>
                <div className="file-value">
                  {importPath ? (
                    <>
                      <span className="file-path">{importPath}</span>
                      <span className={`pc-format-badge ${getFormatBadge(importPath)}`}>
                        {getFormatBadge(importPath)}
                      </span>
                    </>
                  ) : (
                    <span className="file-empty">No file selected</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseImportFile}>Browse</button>
              </div>

              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">VERIFY</span>
                  <span className="file-desc">Optional secondary source for cross-check</span>
                </div>
                <div className="file-value">
                  {secondaryPath ? (
                    <>
                      <span className="file-path">{secondaryPath}</span>
                      <span className={`pc-format-badge ${getFormatBadge(secondaryPath)}`}>
                        {getFormatBadge(secondaryPath)}
                      </span>
                    </>
                  ) : (
                    <span className="file-empty">None (single source mode)</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseSecondaryFile}>Browse</button>
              </div>
            </div>

            {/* Profile + output dir */}
            <div className="pc-format-row">
              <label>Profile:</label>
              <select value={importProfile} onChange={(e) => setImportProfile(e.target.value)}>
                <option value="sonoma">Sonoma — 128ch, 6 cores, Zynq 7020</option>
                <option value="hx">HX — 160ch/axis × 4 axes, 16 supplies, Incal</option>
                <option value="xp160">XP-160/Shasta — 160ch/axis × 8 axes, 32 supplies, Incal</option>
                <option value="mcc">MCC — 128ch, 8 supplies, ISE Labs</option>
              </select>
            </div>

            <div className="profile-files">
              <div className="profile-file">
                <div className="file-info">
                  <span className="file-label">OUTPUT</span>
                  <span className="file-desc">Directory for generated files</span>
                </div>
                <div className="file-value">
                  {importOutputDir ? (
                    <span className="file-path">{importOutputDir}</span>
                  ) : (
                    <span className="file-empty">Select output directory</span>
                  )}
                </div>
                <button className="btn-browse" onClick={browseImportOutputDir}>Browse</button>
              </div>
            </div>

            {/* Extract button */}
            <div className="profile-actions">
              <button
                className="btn-compile"
                onClick={handleExtract}
                disabled={extracting || !importPath}
              >
                {extracting ? 'Extracting...' : 'Extract'}
              </button>
              <button className="btn-clear" onClick={() => {
                setImportPath('')
                setSecondaryPath('')
                setPinTable(null)
                setVerification(null)
                setImportResult(null)
                setError('')
              }}>
                Clear
              </button>
            </div>

            {/* Warnings */}
            {pinTable && pinTable.warnings.length > 0 && (
              <div className="pc-warning-banner warn">
                {pinTable.warnings.map((w, i) => (
                  <div key={i}>{w}</div>
                ))}
              </div>
            )}

            {/* Verification banner */}
            {verification && (
              <div className={`pc-verify-banner ${verification.mismatch_count > 0 ? 'pc-warning-banner warn' : 'pc-warning-banner success'}`}>
                <span>Cross-verification: {verification.match_count} pins compared</span>
                <div className="verify-stats">
                  <span className="stat-match">{verification.match_count - verification.mismatch_count} match</span>
                  {verification.mismatch_count > 0 && (
                    <span className="stat-mismatch">{verification.mismatch_count} mismatches</span>
                  )}
                </div>
              </div>
            )}

            {/* Editable pin table */}
            {pinTable && (
              <>
                <div className="pc-table-section">
                  <h4>Channels ({pinTable.channels.length})</h4>
                  <button className="btn-add" onClick={addChannel}>+ Add Pin</button>
                </div>
                <div className="pc-editable-table-wrap">
                  <table className="pc-editable-table">
                    <thead>
                      <tr>
                        <th style={{ width: 40 }}>#</th>
                        <th>Signal</th>
                        <th style={{ width: 80 }}>Channel</th>
                        <th style={{ width: 100 }}>Direction</th>
                        <th style={{ width: 80 }}>Voltage</th>
                        <th style={{ width: 100 }}>Group</th>
                        <th style={{ width: 60 }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {pinTable.channels.map((ch, idx) => (
                        <tr key={idx}>
                          <td style={{ color: 'var(--text-secondary)', fontSize: 11 }}>{idx + 1}</td>
                          <td className={getMismatch(ch.signal_name, 'missing') ? 'mismatch' : ''}>
                            <input
                              value={ch.signal_name}
                              onChange={(e) => updateChannel(idx, 'signal_name', e.target.value)}
                            />
                          </td>
                          <td className={getMismatch(ch.signal_name, 'channel') ? 'mismatch' : ''}>
                            <input
                              type="number"
                              value={ch.channel}
                              onChange={(e) => updateChannel(idx, 'channel', e.target.value)}
                              title={getMismatch(ch.signal_name, 'channel')
                                ? `Secondary: ${getMismatch(ch.signal_name, 'channel')!.secondary_value}`
                                : undefined}
                            />
                          </td>
                          <td className={getMismatch(ch.signal_name, 'direction') ? 'mismatch' : ''}>
                            <select
                              value={ch.direction}
                              onChange={(e) => updateChannel(idx, 'direction', e.target.value)}
                            >
                              <option value="IO">IO</option>
                              <option value="Input">Input</option>
                              <option value="Output">Output</option>
                            </select>
                          </td>
                          <td className={getMismatch(ch.signal_name, 'voltage') ? 'mismatch' : ''}>
                            <input
                              type="number"
                              step="0.1"
                              value={ch.voltage ?? ''}
                              placeholder="-"
                              onChange={(e) => updateChannel(idx, 'voltage', e.target.value)}
                            />
                          </td>
                          <td>
                            <input
                              value={ch.group ?? ''}
                              placeholder="-"
                              onChange={(e) => updateChannel(idx, 'group', e.target.value)}
                            />
                          </td>
                          <td>
                            <div className="pc-row-actions">
                              <button className="delete" onClick={() => deleteChannel(idx)} title="Delete">x</button>
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

                {/* Supply table */}
                <div className="pc-table-section">
                  <h4>Power Supplies ({pinTable.supplies.length})</h4>
                  <button className="btn-add" onClick={addSupply}>+ Add Supply</button>
                </div>
                <div className="pc-editable-table-wrap" style={{ maxHeight: 200 }}>
                  <table className="pc-editable-table">
                    <thead>
                      <tr>
                        <th>Core</th>
                        <th style={{ width: 80 }}>Voltage</th>
                        <th style={{ width: 80 }}>Sequence</th>
                        <th style={{ width: 100 }}>Ramp (ms)</th>
                        <th style={{ width: 60 }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {pinTable.supplies.map((sup, idx) => (
                        <tr key={idx}>
                          <td>
                            <input
                              value={sup.core_name}
                              onChange={(e) => updateSupply(idx, 'core_name', e.target.value)}
                            />
                          </td>
                          <td>
                            <input
                              type="number"
                              step="0.1"
                              value={sup.voltage}
                              onChange={(e) => updateSupply(idx, 'voltage', e.target.value)}
                            />
                          </td>
                          <td>
                            <input
                              type="number"
                              value={sup.sequence_order}
                              onChange={(e) => updateSupply(idx, 'sequence_order', e.target.value)}
                            />
                          </td>
                          <td>
                            <input
                              type="number"
                              step="1"
                              value={sup.ramp_delay_ms}
                              onChange={(e) => updateSupply(idx, 'ramp_delay_ms', e.target.value)}
                            />
                          </td>
                          <td>
                            <div className="pc-row-actions">
                              <button className="delete" onClick={() => deleteSupply(idx)} title="Delete">x</button>
                            </div>
                          </td>
                        </tr>
                      ))}
                      {pinTable.supplies.length === 0 && (
                        <tr>
                          <td colSpan={5} style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: 16 }}>
                            No supplies defined. Click "+ Add Supply" to add power rails.
                          </td>
                        </tr>
                      )}
                    </tbody>
                  </table>
                </div>

                {/* Generate All */}
                <div className="profile-actions" style={{ marginTop: 16 }}>
                  <button
                    className="btn-compile"
                    onClick={handleGenerateFromExtracted}
                    disabled={importGenerating || !importOutputDir || pinTable.channels.length === 0}
                  >
                    {importGenerating ? 'Generating...' : 'Generate All'}
                  </button>
                </div>

                {/* Generation result */}
                {importResult && (
                  <div className="pc-result">
                    <div className="profile-summary">
                      <span className="summary-value">Generated successfully for {importResult.device_name || pinTable.device_name}</span>
                    </div>
                    <div className="info-cards" style={{ marginTop: 16 }}>
                      <div className="info-card">
                        <div className="card-content">
                          <span className="card-label">Profile</span>
                          <span className="card-value">{importResult.profile}</span>
                        </div>
                      </div>
                      <div className="info-card">
                        <div className="card-content">
                          <span className="card-label">Channels</span>
                          <span className="card-value">{importResult.num_channels}</span>
                        </div>
                      </div>
                      <div className="info-card">
                        <div className="card-content">
                          <span className="card-label">Supplies</span>
                          <span className="card-value">{importResult.num_supplies}</span>
                        </div>
                      </div>
                    </div>
                    <div className="pc-output-files">
                      <h4>Generated Files:</h4>
                      <ul>
                        <li><code>PIN_MAP</code> — channel mapping for pattern converter</li>
                        <li><code>.map</code> — signal to GPIO mapping</li>
                        <li><code>.lvl</code> — bank voltages + VIH/VIL/VOH/VOL</li>
                        <li><code>.tim</code> — period + drive/compare edges</li>
                        <li><code>.tp</code> — test plan steps</li>
                        <li><code>PowerOn.sh</code> — power-on sequence script</li>
                        <li><code>PowerOff.sh</code> — power-off sequence script</li>
                      </ul>
                    </div>
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
