import { useState, useRef, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import './FleetTerminalPanel.css'

interface SshSession {
  id: number
  host: string
  user: string
}

interface ConnectForm {
  host: string
  port: string
  username: string
  password: string
}

export default function FleetTerminalPanel() {
  const [sessions, setSessions] = useState<SshSession[]>([])
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null)
  const [connecting, setConnecting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [form, setForm] = useState<ConnectForm>({
    host: '172.16.0.',
    port: '22',
    username: 'root',
    password: '',
  })

  // Map session_id → { terminal, fitAddon, container div }
  const terminalsRef = useRef<Map<number, { term: Terminal; fit: FitAddon }>>(new Map())
  const containerRef = useRef<HTMLDivElement>(null)
  const unlistenersRef = useRef<Map<number, UnlistenFn[]>>(new Map())

  // Fetch active sessions on mount
  useEffect(() => {
    refreshSessions()
  }, [])

  // Fit terminal on window resize
  useEffect(() => {
    const handleResize = () => {
      const active = activeSessionId !== null ? terminalsRef.current.get(activeSessionId) : null
      if (active) {
        active.fit.fit()
      }
    }
    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
  }, [activeSessionId])

  const refreshSessions = async () => {
    try {
      const list = await invoke<SshSession[]>('ssh_list_sessions')
      setSessions(list)
    } catch (e) {
      console.error('Failed to list sessions:', e)
    }
  }

  const handleConnect = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!form.host.trim()) return

    setConnecting(true)
    setError(null)

    try {
      const sessionId = await invoke<number>('ssh_connect', {
        host: form.host.trim(),
        port: parseInt(form.port) || 22,
        username: form.username.trim() || 'root',
        password: form.password,
      })

      const newSession: SshSession = {
        id: sessionId,
        host: form.host.trim(),
        user: form.username.trim() || 'root',
      }

      setSessions(prev => [...prev, newSession])
      setActiveSessionId(sessionId)

      // Create terminal for this session after DOM update
      setTimeout(() => setupTerminal(sessionId), 0)
    } catch (e) {
      setError(String(e))
    } finally {
      setConnecting(false)
    }
  }

  const setupTerminal = useCallback(async (sessionId: number) => {
    if (terminalsRef.current.has(sessionId)) return

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'Consolas', 'Monaco', 'Courier New', monospace",
      theme: {
        background: '#0d1117',
        foreground: '#c9d1d9',
        cursor: '#4488ff',
        selectionBackground: '#264f78',
        black: '#0d1117',
        red: '#ff7b72',
        green: '#3fb950',
        yellow: '#d29922',
        blue: '#4488ff',
        magenta: '#bc8cff',
        cyan: '#39d353',
        white: '#c9d1d9',
        brightBlack: '#484f58',
        brightRed: '#ffa198',
        brightGreen: '#56d364',
        brightYellow: '#e3b341',
        brightBlue: '#79c0ff',
        brightMagenta: '#d2a8ff',
        brightCyan: '#56d364',
        brightWhite: '#f0f6fc',
      },
    })

    const fit = new FitAddon()
    term.loadAddon(fit)

    terminalsRef.current.set(sessionId, { term, fit })

    // Open terminal into container
    const container = document.getElementById(`terminal-${sessionId}`)
    if (container) {
      term.open(container)
      fit.fit()
    }

    // Send keystrokes to SSH session
    term.onData(async (data: string) => {
      try {
        await invoke('ssh_write', { sessionId, data })
      } catch (e) {
        term.writeln(`\r\n[Connection lost: ${e}]`)
      }
    })

    // Listen for SSH output events
    const unlisteners: UnlistenFn[] = []

    unlisteners.push(
      await listen<string>(`ssh:output:${sessionId}`, (event) => {
        term.write(event.payload)
      })
    )

    unlisteners.push(
      await listen<string>(`ssh:closed:${sessionId}`, (event) => {
        term.writeln(`\r\n[Session closed: ${event.payload}]`)
      })
    )

    unlistenersRef.current.set(sessionId, unlisteners)
  }, [])

  const handleDisconnect = async (sessionId: number) => {
    try {
      await invoke('ssh_disconnect', { sessionId })
    } catch (e) {
      console.error('Disconnect error:', e)
    }

    // Cleanup terminal
    const termEntry = terminalsRef.current.get(sessionId)
    if (termEntry) {
      termEntry.term.dispose()
      terminalsRef.current.delete(sessionId)
    }

    // Cleanup event listeners
    const unlisteners = unlistenersRef.current.get(sessionId)
    if (unlisteners) {
      unlisteners.forEach(fn => fn())
      unlistenersRef.current.delete(sessionId)
    }

    setSessions(prev => prev.filter(s => s.id !== sessionId))
    if (activeSessionId === sessionId) {
      setActiveSessionId(sessions.length > 1 ? sessions.find(s => s.id !== sessionId)?.id ?? null : null)
    }
  }

  const switchToSession = (sessionId: number) => {
    setActiveSessionId(sessionId)
    // Fit terminal after visibility change
    setTimeout(() => {
      const entry = terminalsRef.current.get(sessionId)
      if (entry) {
        entry.fit.fit()
        entry.term.focus()
      }
    }, 0)
  }

  // Cleanup all terminals on unmount
  useEffect(() => {
    return () => {
      terminalsRef.current.forEach((entry) => entry.term.dispose())
      terminalsRef.current.clear()
      unlistenersRef.current.forEach((fns) => fns.forEach(fn => fn()))
      unlistenersRef.current.clear()
    }
  }, [])

  return (
    <div className="fleet-terminal">
      {/* Connect bar */}
      <div className="fleet-connect-bar">
        <form className="connect-form" onSubmit={handleConnect}>
          <input
            type="text"
            className="connect-input host"
            placeholder="172.16.0.x"
            value={form.host}
            onChange={e => setForm(f => ({ ...f, host: e.target.value }))}
          />
          <input
            type="text"
            className="connect-input port"
            placeholder="22"
            value={form.port}
            onChange={e => setForm(f => ({ ...f, port: e.target.value }))}
          />
          <input
            type="text"
            className="connect-input user"
            placeholder="root"
            value={form.username}
            onChange={e => setForm(f => ({ ...f, username: e.target.value }))}
          />
          <input
            type="password"
            className="connect-input pass"
            placeholder="password"
            value={form.password}
            onChange={e => setForm(f => ({ ...f, password: e.target.value }))}
          />
          <button
            type="submit"
            className="connect-btn"
            disabled={connecting || !form.host.trim()}
          >
            {connecting ? 'Connecting...' : 'Connect'}
          </button>
        </form>
        {error && <div className="connect-error">{error}</div>}
      </div>

      {/* Session tabs */}
      {sessions.length > 0 && (
        <div className="session-tabs">
          {sessions.map(s => (
            <div
              key={s.id}
              className={`session-tab ${s.id === activeSessionId ? 'active' : ''}`}
              onClick={() => switchToSession(s.id)}
            >
              <span className="tab-label">{s.user}@{s.host}</span>
              <button
                className="tab-close"
                onClick={(e) => { e.stopPropagation(); handleDisconnect(s.id) }}
                title="Disconnect"
              >
                x
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Terminal area */}
      <div className="terminal-area" ref={containerRef}>
        {sessions.length === 0 ? (
          <div className="no-sessions">
            <div className="no-sessions-text">No active sessions</div>
            <div className="no-sessions-hint">
              Enter a host address above to connect via SSH
            </div>
          </div>
        ) : (
          sessions.map(s => (
            <div
              key={s.id}
              id={`terminal-${s.id}`}
              className={`terminal-container ${s.id === activeSessionId ? 'visible' : 'hidden'}`}
            />
          ))
        )}
      </div>
    </div>
  )
}
