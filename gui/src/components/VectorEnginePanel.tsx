import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './VectorEnginePanel.css'

// Backend response type
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
  const { selectedBoard, connected } = useStore()
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

  // Fetch status from backend
  const fetchStatus = useCallback(async () => {
    if (!selectedBoard || !connected) return
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
  }, [selectedBoard, connected, lastVectorCount, lastTime, status.vectors_per_sec])

  // Poll status when running or paused
  useEffect(() => {
    if (!selectedBoard || !connected) return
    if (status.state !== 'running' && status.state !== 'paused') return

    const interval = setInterval(fetchStatus, 100)
    return () => clearInterval(interval)
  }, [selectedBoard, connected, status.state, fetchStatus])

  // Initial status fetch
  useEffect(() => {
    if (selectedBoard && connected) {
      fetchStatus()
    }
  }, [selectedBoard, connected])

  // Handle file selection via native dialog (invokes backend)
  const handleBrowseFile = async () => {
    try {
      // This would invoke a backend command that shows a native file dialog
      // For now, add a sample file to the list for demo purposes
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
    if (!selectedBoard) return

    setStatus(prev => ({ ...prev, state: 'loading' }))
    try {
      // Call backend to load vectors from file path
      // Backend will read the file and send to board
      await invoke('upload_vectors', { mac: selectedBoard, data: [] }) // Use upload_vectors as placeholder
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

  const handleStart = async () => {
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
    if (!selectedBoard) return
    try {
      await invoke('pause_vectors', { mac: selectedBoard })
      setStatus(prev => ({ ...prev, state: 'paused' }))
    } catch (e) {
      console.error('Failed to pause:', e)
    }
  }

  const handleResume = async () => {
    if (!selectedBoard) return
    try {
      await invoke('resume_vectors', { mac: selectedBoard })
      setStatus(prev => ({ ...prev, state: 'running' }))
    } catch (e) {
      console.error('Failed to resume:', e)
    }
  }

  const handleStop = async () => {
    if (!selectedBoard) return
    try {
      await invoke('stop_vectors', { mac: selectedBoard })
      setStatus(prev => ({ ...prev, state: 'idle' }))
    } catch (e) {
      console.error('Failed to stop:', e)
    }
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

  if (!selectedBoard) {
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
        </div>
        <div className="header-actions">
          {(status.state === 'idle' || status.state === 'ready') && (
            <button
              className="btn-start"
              onClick={handleStart}
              disabled={!selectedFile || status.state !== 'ready'}
            >
              Start
            </button>
          )}
          {status.state === 'running' && (
            <>
              <button className="btn-pause" onClick={handlePause}>
                Pause
              </button>
              <button className="btn-stop" onClick={handleStop}>
                Stop
              </button>
            </>
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
          </div>
        </div>

        {/* Vector Files */}
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
      </div>
    </div>
  )
}
