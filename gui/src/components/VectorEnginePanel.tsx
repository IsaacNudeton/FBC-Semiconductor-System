import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './VectorEnginePanel.css'

// FBC backend response type
interface VectorStatusResponse {
  state: string
  current_address: number
  total_vectors: number
  loop_count: number
  target_loops: number
  error_count: number
  first_fail_addr: number
  run_time_ms: number
}

// Sonoma run result
interface SonomaRunResult {
  passed: boolean
  vectors_executed: number
  errors: number
  duration_s: number
}

// UI display type
interface VectorStatus {
  state: 'idle' | 'loading' | 'ready' | 'running' | 'paused' | 'done' | 'error'
  current_vector: number
  total_vectors: number
  loop_count: number
  total_loops: number
  errors_detected: number
  run_time_ms: number
  vectors_per_sec: number
  first_fail_addr: number
}

interface VectorFile {
  name: string
  path: string
  size: number
  vectors: number
  loaded: boolean
}

export default function VectorEnginePanel() {
  const { selectedBoard, connected, controlMode, selectedSonomaBoard } = useStore()
  const [status, setStatus] = useState<VectorStatus>({
    state: 'idle',
    current_vector: 0,
    total_vectors: 0,
    loop_count: 0,
    total_loops: 1,
    errors_detected: 0,
    run_time_ms: 0,
    vectors_per_sec: 0,
    first_fail_addr: 0,
  })
  const [vectorFiles, setVectorFiles] = useState<VectorFile[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [loopCount, setLoopCount] = useState(1)
  const [stopOnError, setStopOnError] = useState(true)
  const [lastVectorCount, setLastVectorCount] = useState(0)
  const [lastTime, setLastTime] = useState(0)

  // Sonoma-specific state
  const [sonomaSeqPath, setSonomaSeqPath] = useState('')
  const [sonomaHexPath, setSonomaHexPath] = useState('')
  const [sonomaRunTime, setSonomaRunTime] = useState(60)

  const hasBoard = controlMode === 'fbc' ? (selectedBoard && connected) : !!selectedSonomaBoard

  // Parse backend state to UI state
  const parseState = (state: string): VectorStatus['state'] => {
    switch (state.toLowerCase()) {
      case 'idle': return 'idle'
      case 'loading': return 'loading'
      case 'ready': return 'ready'
      case 'running': return 'running'
      case 'paused': return 'paused'
      case 'done': return 'done'
      case 'error': return 'error'
      default: return 'idle'
    }
  }

  // Fetch status from backend (FBC only — Sonoma is fire-and-forget)
  const fetchStatus = useCallback(async () => {
    if (!selectedBoard || !connected || controlMode !== 'fbc') return
    try {
      const result = await invoke<VectorStatusResponse>('get_vector_status', { mac: selectedBoard })
      const now = Date.now()
      const vectorsDelta = result.current_address - lastVectorCount
      const timeDelta = now - lastTime
      const vectorsPerSec = timeDelta > 0 ? Math.round((vectorsDelta * 1000) / timeDelta) : 0

      setLastVectorCount(result.current_address)
      setLastTime(now)

      setStatus({
        state: parseState(result.state),
        current_vector: result.current_address,
        total_vectors: result.total_vectors,
        loop_count: result.loop_count,
        total_loops: result.target_loops,
        errors_detected: result.error_count,
        run_time_ms: Number(result.run_time_ms),
        vectors_per_sec: vectorsPerSec > 0 ? vectorsPerSec : status.vectors_per_sec,
        first_fail_addr: result.first_fail_addr,
      })
    } catch (e) {
      console.error('Failed to get status:', e)
    }
  }, [selectedBoard, connected, lastVectorCount, lastTime, status.vectors_per_sec, controlMode])

  // Poll status when running or paused (FBC only)
  useEffect(() => {
    if (!selectedBoard || !connected || controlMode !== 'fbc') return
    if (status.state !== 'running' && status.state !== 'paused') return

    const interval = setInterval(fetchStatus, 100)
    return () => clearInterval(interval)
  }, [selectedBoard, connected, status.state, fetchStatus, controlMode])

  // Initial status fetch (FBC only)
  useEffect(() => {
    if (selectedBoard && connected && controlMode === 'fbc') {
      fetchStatus()
    }
  }, [selectedBoard, connected, controlMode])

  // Handle file selection
  const handleBrowseFile = async () => {
    try {
      const demoPath = 'C:/vectors/test.fbc'
      const name = 'test.fbc'
      setVectorFiles(prev => [
        ...prev.filter(f => f.path !== demoPath),
        { name, path: demoPath, size: 102400, vectors: 5000, loaded: false }
      ])
    } catch (e) {
      console.error('Failed to browse file:', e)
    }
  }

  const handleLoadFile = async (file: VectorFile) => {
    if (!selectedBoard && controlMode === 'fbc') return

    setStatus(prev => ({ ...prev, state: 'loading' }))
    try {
      if (controlMode === 'fbc') {
        await invoke('upload_vectors', { mac: selectedBoard, data: [] })
      }
      setVectorFiles(prev => prev.map(f =>
        f.path === file.path ? { ...f, loaded: true } : { ...f, loaded: false }
      ))
      setSelectedFile(file.path)
      setStatus({
        state: 'ready',
        current_vector: 0,
        total_vectors: file.vectors,
        loop_count: 0,
        total_loops: loopCount,
        errors_detected: 0,
        run_time_ms: 0,
        vectors_per_sec: 0,
        first_fail_addr: 0,
      })
    } catch (e) {
      console.error('Failed to load vectors:', e)
      setStatus(prev => ({ ...prev, state: 'error' }))
    }
  }

  // Sonoma: Load vectors from paths on the board
  const handleSonomaLoad = async () => {
    if (!selectedSonomaBoard || !sonomaSeqPath || !sonomaHexPath) return

    setStatus(prev => ({ ...prev, state: 'loading' }))
    try {
      await invoke('sonoma_load_vectors', {
        ip: selectedSonomaBoard,
        seqPath: sonomaSeqPath,
        hexPath: sonomaHexPath,
      })
      setStatus(prev => ({ ...prev, state: 'ready' }))
    } catch (e) {
      console.error('Failed to load vectors:', e)
      setStatus(prev => ({ ...prev, state: 'error' }))
    }
  }

  // Sonoma: Run vectors
  const handleSonomaRun = async () => {
    if (!selectedSonomaBoard || !sonomaSeqPath) return

    setStatus(prev => ({ ...prev, state: 'running' }))
    try {
      const result = await invoke<SonomaRunResult>('sonoma_run_vectors', {
        ip: selectedSonomaBoard,
        seqPath: sonomaSeqPath,
        timeS: sonomaRunTime,
        debug: false,
      })
      setStatus(prev => ({
        ...prev,
        state: result.passed ? 'done' : 'error',
        errors_detected: result.errors,
        run_time_ms: result.duration_s * 1000,
      }))
    } catch (e) {
      console.error('Failed to run vectors:', e)
      setStatus(prev => ({ ...prev, state: 'error' }))
    }
  }

  const handleStart = async () => {
    if (controlMode === 'sonoma') {
      handleSonomaRun()
      return
    }
    if (!selectedBoard || !selectedFile) return

    try {
      await invoke('start_vectors', {
        mac: selectedBoard,
        loops: loopCount,
      })
      setStatus(prev => ({ ...prev, state: 'running', total_loops: loopCount }))
      setLastVectorCount(0)
      setLastTime(Date.now())
    } catch (e) {
      console.error('Failed to start:', e)
    }
  }

  const handlePause = async () => {
    if (!selectedBoard || controlMode !== 'fbc') return
    try {
      await invoke('pause_vectors', { mac: selectedBoard })
      setStatus(prev => ({ ...prev, state: 'paused' }))
    } catch (e) {
      console.error('Failed to pause:', e)
    }
  }

  const handleResume = async () => {
    if (!selectedBoard || controlMode !== 'fbc') return
    try {
      await invoke('resume_vectors', { mac: selectedBoard })
      setStatus(prev => ({ ...prev, state: 'running' }))
    } catch (e) {
      console.error('Failed to resume:', e)
    }
  }

  const handleStop = async () => {
    if (controlMode === 'fbc' && selectedBoard) {
      try {
        await invoke('stop_vectors', { mac: selectedBoard })
      } catch (e) {
        console.error('Failed to stop:', e)
      }
    }
    setStatus(prev => ({ ...prev, state: 'idle' }))
  }

  const formatTime = (ms: number): string => {
    const seconds = Math.floor(ms / 1000)
    const minutes = Math.floor(seconds / 60)
    const hours = Math.floor(minutes / 60)
    if (hours > 0) {
      return `${hours}:${(minutes % 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`
    }
    return `${minutes}:${(seconds % 60).toString().padStart(2, '0')}`
  }

  const formatNumber = (n: number): string => {
    return n.toLocaleString()
  }

  const progress = status.total_vectors > 0
    ? (status.current_vector / status.total_vectors) * 100
    : 0

  const loopProgress = status.total_loops > 0
    ? (status.loop_count / status.total_loops) * 100
    : 0

  if (!hasBoard) {
    return (
      <div className="vector-engine-panel">
        <div className="no-board-message">
          <span className="icon">🔄</span>
          <h3>No Board Selected</h3>
          <p>Select a board to control the vector engine.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="vector-engine-panel">
      {/* Header */}
      <div className="vector-header">
        <div className="header-info">
          <h2>Vector Engine</h2>
          <div className={`state-indicator state-${status.state}`}>
            <span className="state-dot" />
            {status.state.charAt(0).toUpperCase() + status.state.slice(1)}
          </div>
          {controlMode === 'sonoma' && (
            <span style={{ fontSize: 11, color: 'var(--accent)', marginLeft: 8 }}>SSH</span>
          )}
        </div>
        <div className="header-actions">
          {(status.state === 'idle' || status.state === 'ready') && (
            <button
              className="btn-start"
              onClick={handleStart}
              disabled={controlMode === 'fbc' ? (!selectedFile || status.state !== 'ready') : !sonomaSeqPath}
            >
              Start
            </button>
          )}
          {status.state === 'running' && controlMode === 'fbc' && (
            <>
              <button className="btn-pause" onClick={handlePause}>
                Pause
              </button>
              <button className="btn-stop" onClick={handleStop}>
                Stop
              </button>
            </>
          )}
          {status.state === 'running' && controlMode === 'sonoma' && (
            <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>Running on board...</span>
          )}
          {status.state === 'paused' && (
            <>
              <button className="btn-resume" onClick={handleResume}>
                Resume
              </button>
              <button className="btn-stop" onClick={handleStop}>
                Stop
              </button>
            </>
          )}
          {(status.state === 'done' || status.state === 'error') && (
            <button className="btn-reset" onClick={() => setStatus(prev => ({ ...prev, state: 'idle' }))}>
              Reset
            </button>
          )}
        </div>
      </div>

      <div className="vector-content">
        {/* Progress Section */}
        <div className="progress-section">
          <div className="progress-card">
            <div className="progress-header">
              <span className="progress-label">Vector Progress</span>
              <span className="progress-value">
                {formatNumber(status.current_vector)} / {formatNumber(status.total_vectors)}
              </span>
            </div>
            <div className="progress-bar">
              <div className="progress-fill" style={{ width: `${progress}%` }} />
            </div>
            <div className="progress-percent">{progress.toFixed(1)}%</div>
          </div>

          <div className="progress-card">
            <div className="progress-header">
              <span className="progress-label">Loop Progress</span>
              <span className="progress-value">
                {status.loop_count} / {status.total_loops}
              </span>
            </div>
            <div className="progress-bar">
              <div className="progress-fill loop" style={{ width: `${loopProgress}%` }} />
            </div>
            <div className="progress-percent">{loopProgress.toFixed(1)}%</div>
          </div>
        </div>

        {/* Stats Grid */}
        <div className="stats-section">
          <div className="stat-card">
            <span className="stat-icon">⏱️</span>
            <div className="stat-info">
              <span className="stat-value">{formatTime(status.run_time_ms)}</span>
              <span className="stat-label">Run Time</span>
            </div>
          </div>
          <div className="stat-card">
            <span className="stat-icon">⚡</span>
            <div className="stat-info">
              <span className="stat-value">{formatNumber(status.vectors_per_sec)}</span>
              <span className="stat-label">Vectors/sec</span>
            </div>
          </div>
          <div className={`stat-card ${status.errors_detected > 0 ? 'error' : ''}`}>
            <span className="stat-icon">❌</span>
            <div className="stat-info">
              <span className="stat-value">{formatNumber(status.errors_detected)}</span>
              <span className="stat-label">Errors</span>
            </div>
          </div>
        </div>

        {/* Configuration */}
        <div className="config-section">
          <h3>Configuration</h3>
          <div className="config-grid">
            {controlMode === 'fbc' ? (
              <>
                <div className="config-item">
                  <label>Loop Count</label>
                  <input
                    type="number"
                    value={loopCount}
                    onChange={e => setLoopCount(Math.max(1, parseInt(e.target.value) || 1))}
                    min={1}
                    max={999999}
                    disabled={status.state !== 'idle' && status.state !== 'ready'}
                  />
                </div>
                <div className="config-item checkbox">
                  <label>
                    <input
                      type="checkbox"
                      checked={stopOnError}
                      onChange={e => setStopOnError(e.target.checked)}
                      disabled={status.state !== 'idle' && status.state !== 'ready'}
                    />
                    Stop on Error
                  </label>
                </div>
              </>
            ) : (
              <>
                <div className="config-item">
                  <label>.seq path (on board)</label>
                  <input
                    type="text"
                    value={sonomaSeqPath}
                    onChange={e => setSonomaSeqPath(e.target.value)}
                    placeholder="/home/DeviceName/test.seq"
                    disabled={status.state === 'running'}
                  />
                </div>
                <div className="config-item">
                  <label>.hex path (on board)</label>
                  <input
                    type="text"
                    value={sonomaHexPath}
                    onChange={e => setSonomaHexPath(e.target.value)}
                    placeholder="/home/DeviceName/test.hex"
                    disabled={status.state === 'running'}
                  />
                </div>
                <div className="config-item">
                  <label>Run Time (seconds)</label>
                  <input
                    type="number"
                    value={sonomaRunTime}
                    onChange={e => setSonomaRunTime(Math.max(1, parseInt(e.target.value) || 60))}
                    min={1}
                    max={86400}
                    disabled={status.state === 'running'}
                  />
                </div>
                <div className="config-item">
                  <button
                    className="btn-start"
                    onClick={handleSonomaLoad}
                    disabled={!sonomaSeqPath || !sonomaHexPath || status.state === 'running'}
                    style={{ marginTop: 4 }}
                  >
                    Load Vectors
                  </button>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Vector Files (FBC mode only) */}
        {controlMode === 'fbc' && (
          <div className="files-section">
            <div className="files-header">
              <h3>Vector Files</h3>
              <button className="btn-browse" onClick={handleBrowseFile}>Browse...</button>
            </div>
            <div className="file-list">
              {vectorFiles.map(file => (
                <div
                  key={file.path}
                  className={`file-item ${file.loaded ? 'loaded' : ''} ${selectedFile === file.path ? 'selected' : ''}`}
                  onClick={() => !file.loaded && handleLoadFile(file)}
                >
                  <div className="file-icon">📄</div>
                  <div className="file-info">
                    <span className="file-name">{file.name}</span>
                    <span className="file-meta">
                      {formatNumber(file.vectors)} vectors • {(file.size / 1024).toFixed(0)} KB
                    </span>
                  </div>
                  <div className="file-status">
                    {file.loaded ? (
                      <span className="loaded-badge">Loaded</span>
                    ) : (
                      <button className="btn-load">Load</button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
