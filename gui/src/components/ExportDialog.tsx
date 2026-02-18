import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { useStore } from '../store'
import './ExportDialog.css'

interface ExportOptions {
  format: string
  include_raw_data: boolean
  include_waveforms: boolean
  time_range: string
  custom_start: number | null
  custom_end: number | null
}

interface ExportStats {
  rows_exported: number
  file_size_bytes: number
  duration_ms: number
}

interface TestResult {
  timestamp: number
  mac: string
  board_id: number
  serial: number
  state: string
  cycles: number
  errors: number
  temp_c: number
  duration_ms: number
  vectors_loaded: number
  vectors_executed: number
  error_rate_ppm: number
}

interface Props {
  isOpen: boolean
  onClose: () => void
}

export default function ExportDialog({ isOpen, onClose }: Props) {
  const { boards, getTelemetryHistory, getTestRuns } = useStore()

  const [format, setFormat] = useState('csv')
  const [includeRawData, setIncludeRawData] = useState(false)
  const [includeWaveforms, setIncludeWaveforms] = useState(false)
  const [timeRange, setTimeRange] = useState('all')
  const [customStart, setCustomStart] = useState('')
  const [customEnd, setCustomEnd] = useState('')
  const [exporting, setExporting] = useState(false)
  const [exportResult, setExportResult] = useState<ExportStats | null>(null)
  const [error, setError] = useState<string | null>(null)

  if (!isOpen) return null

  // Gather all test results from all boards
  const gatherResults = (): TestResult[] => {
    const results: TestResult[] = []

    for (const board of boards) {
      const testRuns = getTestRuns(board.mac)
      const history = getTelemetryHistory(board.mac)

      // Convert test runs to TestResult format
      for (const run of testRuns) {
        results.push({
          timestamp: run.start_time,
          mac: board.mac,
          board_id: board.serial,
          serial: board.serial,
          state: run.final_state,
          cycles: run.total_cycles,
          errors: run.total_errors,
          temp_c: run.avg_temp_c,
          duration_ms: run.end_time - run.start_time,
          vectors_loaded: 0,
          vectors_executed: run.total_cycles,
          error_rate_ppm: run.total_cycles > 0
            ? (run.total_errors * 1_000_000) / run.total_cycles
            : 0,
        })
      }

      // Also include latest telemetry as a snapshot
      if (history.length > 0) {
        const latest = history[history.length - 1]
        results.push({
          timestamp: latest.timestamp,
          mac: board.mac,
          board_id: board.serial,
          serial: board.serial,
          state: latest.state,
          cycles: latest.cycles,
          errors: latest.errors,
          temp_c: latest.temp_c,
          duration_ms: 0,
          vectors_loaded: 0,
          vectors_executed: latest.cycles,
          error_rate_ppm: latest.cycles > 0
            ? (latest.errors * 1_000_000) / latest.cycles
            : 0,
        })
      }
    }

    return results
  }

  const handleExport = async () => {
    setExporting(true)
    setError(null)
    setExportResult(null)

    try {
      // Get file extension based on format
      const extensions = {
        csv: ['csv'],
        json: ['json'],
        stdf: ['stdf'],
      }

      const filterName = {
        csv: 'CSV Files',
        json: 'JSON Files',
        stdf: 'STDF Files',
      }

      const path = await save({
        filters: [
          { name: filterName[format as keyof typeof filterName] || 'All Files', extensions: extensions[format as keyof typeof extensions] || ['*'] },
        ],
        defaultPath: `fbc_results_${new Date().toISOString().slice(0, 10)}.${format}`,
      })

      if (!path) {
        setExporting(false)
        return
      }

      const results = gatherResults()

      const options: ExportOptions = {
        format,
        include_raw_data: includeRawData,
        include_waveforms: includeWaveforms,
        time_range: timeRange,
        custom_start: timeRange === 'custom' && customStart ? new Date(customStart).getTime() : null,
        custom_end: timeRange === 'custom' && customEnd ? new Date(customEnd).getTime() : null,
      }

      const stats = await invoke<ExportStats>('export_results', {
        options,
        outputPath: path,
        results,
      })

      setExportResult(stats)
    } catch (e) {
      setError(String(e))
    }

    setExporting(false)
  }

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  return (
    <div className="export-overlay" onClick={onClose}>
      <dialog className="export-dialog" open onClick={(e) => e.stopPropagation()}>
        <h2>Export Results</h2>

        {exportResult ? (
          <div className="export-success">
            <div className="success-icon">&#10003;</div>
            <h3>Export Complete</h3>
            <div className="export-stats">
              <div className="stat">
                <span className="label">Rows exported:</span>
                <span className="value">{exportResult.rows_exported.toLocaleString()}</span>
              </div>
              <div className="stat">
                <span className="label">File size:</span>
                <span className="value">{formatFileSize(exportResult.file_size_bytes)}</span>
              </div>
              <div className="stat">
                <span className="label">Duration:</span>
                <span className="value">{exportResult.duration_ms} ms</span>
              </div>
            </div>
            <div className="actions">
              <button onClick={onClose} className="primary">Done</button>
            </div>
          </div>
        ) : (
          <>
            <fieldset>
              <legend>Format</legend>
              <label className={format === 'csv' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="format"
                  value="csv"
                  checked={format === 'csv'}
                  onChange={(e) => setFormat(e.target.value)}
                />
                <div className="format-info">
                  <span className="format-name">CSV</span>
                  <span className="format-desc">Excel compatible, easy to analyze</span>
                </div>
              </label>
              <label className={format === 'json' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="format"
                  value="json"
                  checked={format === 'json'}
                  onChange={(e) => setFormat(e.target.value)}
                />
                <div className="format-info">
                  <span className="format-name">JSON</span>
                  <span className="format-desc">API compatible, structured data</span>
                </div>
              </label>
              <label className={format === 'stdf' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="format"
                  value="stdf"
                  checked={format === 'stdf'}
                  onChange={(e) => setFormat(e.target.value)}
                />
                <div className="format-info">
                  <span className="format-name">STDF</span>
                  <span className="format-desc">Industry standard test data format</span>
                </div>
              </label>
            </fieldset>

            <fieldset>
              <legend>Options</legend>
              <label>
                <input
                  type="checkbox"
                  checked={includeRawData}
                  onChange={(e) => setIncludeRawData(e.target.checked)}
                />
                Include raw vector data
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={includeWaveforms}
                  onChange={(e) => setIncludeWaveforms(e.target.checked)}
                />
                Include waveform captures
              </label>
            </fieldset>

            <fieldset>
              <legend>Time Range</legend>
              <label className={timeRange === 'all' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="range"
                  value="all"
                  checked={timeRange === 'all'}
                  onChange={(e) => setTimeRange(e.target.value)}
                />
                All data
              </label>
              <label className={timeRange === 'last-hour' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="range"
                  value="last-hour"
                  checked={timeRange === 'last-hour'}
                  onChange={(e) => setTimeRange(e.target.value)}
                />
                Last hour
              </label>
              <label className={timeRange === 'custom' ? 'selected' : ''}>
                <input
                  type="radio"
                  name="range"
                  value="custom"
                  checked={timeRange === 'custom'}
                  onChange={(e) => setTimeRange(e.target.value)}
                />
                Custom range
              </label>

              {timeRange === 'custom' && (
                <div className="custom-range">
                  <label>
                    Start:
                    <input
                      type="datetime-local"
                      value={customStart}
                      onChange={(e) => setCustomStart(e.target.value)}
                    />
                  </label>
                  <label>
                    End:
                    <input
                      type="datetime-local"
                      value={customEnd}
                      onChange={(e) => setCustomEnd(e.target.value)}
                    />
                  </label>
                </div>
              )}
            </fieldset>

            <div className="data-summary">
              <span className="icon">&#128202;</span>
              <span>{boards.length} boards, {gatherResults().length} records available</span>
            </div>

            {error && (
              <div className="error-message">
                <span className="icon">&#9888;</span>
                {error}
              </div>
            )}

            <div className="actions">
              <button onClick={onClose} disabled={exporting}>Cancel</button>
              <button
                onClick={handleExport}
                className="primary"
                disabled={exporting || boards.length === 0}
              >
                {exporting ? 'Exporting...' : 'Export'}
              </button>
            </div>
          </>
        )}
      </dialog>
    </div>
  )
}
