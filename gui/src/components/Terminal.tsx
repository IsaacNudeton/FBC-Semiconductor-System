import { useState, useRef, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import './Terminal.css'

interface TerminalProps {
  isOpen: boolean
  onToggle: () => void
}

interface HistoryEntry {
  type: 'input' | 'output' | 'error'
  text: string
}

export default function Terminal({ isOpen, onToggle }: TerminalProps) {
  const [input, setInput] = useState('')
  const [history, setHistory] = useState<HistoryEntry[]>([
    { type: 'output', text: 'FBC System Terminal v1.0' },
    { type: 'output', text: "Type 'help' for available commands.\n" },
  ])
  const [commandHistory, setCommandHistory] = useState<string[]>([])
  const [historyIndex, setHistoryIndex] = useState(-1)
  const inputRef = useRef<HTMLInputElement>(null)
  const outputRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight
    }
  }, [history])

  // Focus input when terminal opens
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus()
    }
  }, [isOpen])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim()) return

    const cmd = input.trim()
    setHistory((h) => [...h, { type: 'input', text: `> ${cmd}` }])
    setCommandHistory((h) => [...h, cmd])
    setHistoryIndex(-1)
    setInput('')

    try {
      const result = await invoke<string>('terminal_command', { command: cmd })
      if (result) {
        setHistory((h) => [...h, { type: 'output', text: result }])
      }
    } catch (e) {
      setHistory((h) => [
        ...h,
        { type: 'error', text: `Error: ${e}` },
      ])
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (commandHistory.length > 0) {
        const newIndex =
          historyIndex === -1
            ? commandHistory.length - 1
            : Math.max(0, historyIndex - 1)
        setHistoryIndex(newIndex)
        setInput(commandHistory[newIndex])
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (historyIndex !== -1) {
        const newIndex = historyIndex + 1
        if (newIndex >= commandHistory.length) {
          setHistoryIndex(-1)
          setInput('')
        } else {
          setHistoryIndex(newIndex)
          setInput(commandHistory[newIndex])
        }
      }
    }
  }

  return (
    <div className={`terminal ${isOpen ? 'open' : 'closed'}`}>
      <div className="terminal-header" onClick={onToggle}>
        <span className="terminal-title">Terminal</span>
        <span className="terminal-toggle">{isOpen ? '▼' : '▲'}</span>
      </div>

      {isOpen && (
        <div className="terminal-body">
          <div className="terminal-output" ref={outputRef}>
            {history.map((entry, i) => (
              <div key={i} className={`terminal-line ${entry.type}`}>
                {entry.text.split('\n').map((line, j) => (
                  <div key={j}>{line || ' '}</div>
                ))}
              </div>
            ))}
          </div>

          <form className="terminal-input-form" onSubmit={handleSubmit}>
            <span className="terminal-prompt">$</span>
            <input
              ref={inputRef}
              type="text"
              className="terminal-input"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Enter command..."
              autoComplete="off"
              spellCheck={false}
            />
          </form>
        </div>
      )}
    </div>
  )
}
