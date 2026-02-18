import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './Toolbar.css'

export default function Toolbar() {
  const {
    connected,
    currentInterface,
    interfaces,
    setConnected,
    setCurrentInterface,
    setInterfaces,
    setBoards,
    setSelectedBoard,
  } = useStore()

  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Fetch available interfaces on mount
  useEffect(() => {
    const fetchInterfaces = async () => {
      try {
        const ifaces = await invoke<string[]>('list_interfaces')
        setInterfaces(ifaces)
        // Auto-select first interface if none selected
        if (ifaces.length > 0 && !currentInterface) {
          setCurrentInterface(ifaces[0])
        }
      } catch (e) {
        console.error('Failed to get interfaces:', e)
      }
    }
    fetchInterfaces()
  }, [])

  const handleConnect = async () => {
    if (!currentInterface) {
      setError('Select a network interface')
      return
    }

    setLoading(true)
    setError(null)

    try {
      await invoke('connect', { interface: currentInterface })
      setConnected(true)

      // Discover boards after connecting
      const boards = await invoke<any[]>('discover_boards')
      setBoards(boards)
    } catch (e) {
      setError(`Connection failed: ${e}`)
      setConnected(false)
    }

    setLoading(false)
  }

  const handleDisconnect = async () => {
    setLoading(true)
    try {
      await invoke('disconnect')
    } catch (e) {
      console.error('Disconnect error:', e)
    }
    setConnected(false)
    setBoards([])
    setSelectedBoard(null)
    setLoading(false)
  }

  const handleRefresh = async () => {
    if (!connected) return
    setLoading(true)
    try {
      const boards = await invoke<any[]>('discover_boards')
      setBoards(boards)
    } catch (e) {
      setError(`Refresh failed: ${e}`)
    }
    setLoading(false)
  }

  return (
    <div className="toolbar">
      <div className="toolbar-section">
        <span className="toolbar-label">Interface:</span>
        <select
          className="toolbar-select"
          value={currentInterface || ''}
          onChange={(e) => setCurrentInterface(e.target.value || null)}
          disabled={connected || loading}
        >
          <option value="">Select...</option>
          {interfaces.map((iface) => (
            <option key={iface} value={iface}>
              {iface}
            </option>
          ))}
        </select>
      </div>

      <div className="toolbar-section">
        {!connected ? (
          <button
            className="toolbar-btn btn-connect"
            onClick={handleConnect}
            disabled={loading || !currentInterface}
          >
            {loading ? 'Connecting...' : 'Connect'}
          </button>
        ) : (
          <>
            <button
              className="toolbar-btn btn-refresh"
              onClick={handleRefresh}
              disabled={loading}
            >
              Refresh
            </button>
            <button
              className="toolbar-btn btn-disconnect"
              onClick={handleDisconnect}
              disabled={loading}
            >
              Disconnect
            </button>
          </>
        )}
      </div>

      <div className="toolbar-status">
        <span className={`status-indicator ${connected ? 'connected' : 'disconnected'}`} />
        <span className="status-text">{connected ? 'Connected' : 'Disconnected'}</span>
      </div>

      {error && <div className="toolbar-error">{error}</div>}

      <div className="toolbar-spacer" />

      <div className="toolbar-brand">
        <span className="brand-text">FBC System</span>
        <span className="brand-version">v1.0</span>
      </div>
    </div>
  )
}
