import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'
import './FastPinsPanel.css'

interface FastPinState {
    dout: number    // 32-bit drive values
    oen: number     // 32-bit output enables
    din: number     // 32-bit actual pin states
}

export default function FastPinsPanel() {
    const { selectedBoard, connected } = useStore()
    const [pinState, setPinState] = useState<FastPinState | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    // Fetch pin state when board is selected
    useEffect(() => {
        if (!selectedBoard || !connected) {
            setPinState(null)
            return
        }

        const fetchPinState = async () => {
            try {
                const state = await invoke<FastPinState>('get_fast_pins', { mac: selectedBoard })
                setPinState(state)
                setError(null)
            } catch (e) {
                console.error('Failed to get fast pin state:', e)
                setError(String(e))
            }
        }

        fetchPinState()
        const interval = setInterval(fetchPinState, 1000)
        return () => clearInterval(interval)
    }, [selectedBoard, connected])

    const togglePin = async (pin: number, field: 'dout' | 'oen') => {
        if (!selectedBoard || !pinState) return

        const currentValue = field === 'dout' ? pinState.dout : pinState.oen
        const newValue = currentValue ^ (1 << pin) // Toggle bit

        setLoading(true)
        try {
            const newDout = field === 'dout' ? newValue : pinState.dout
            const newOen = field === 'oen' ? newValue : pinState.oen
            await invoke('set_fast_pins', { mac: selectedBoard, dout: newDout, oen: newOen })
            setPinState({ ...pinState, dout: newDout, oen: newOen })
        } catch (e) {
            console.error('Failed to set fast pins:', e)
            setError(String(e))
        }
        setLoading(false)
    }

    const setAllPins = async (dout: number, oen: number) => {
        if (!selectedBoard) return

        setLoading(true)
        try {
            await invoke('set_fast_pins', { mac: selectedBoard, dout, oen })
            setPinState((prev) => prev ? { ...prev, dout, oen } : null)
        } catch (e) {
            console.error('Failed to set fast pins:', e)
            setError(String(e))
        }
        setLoading(false)
    }

    const getBit = (value: number, bit: number): boolean => {
        return ((value >>> bit) & 1) === 1
    }

    const getPinClass = (pin: number): string => {
        if (!pinState) return 'pin-unknown'
        const isOutput = getBit(pinState.oen, pin)
        const driveValue = getBit(pinState.dout, pin)
        if (!isOutput) return 'pin-input'
        return driveValue ? 'pin-high' : 'pin-low'
    }

    return (
        <div className="fast-pins-panel">
            <h2>Fast Pins (Bank 35)</h2>
            <p className="description">Direct FPGA pins gpio[128:159] - 1-cycle latency</p>

            {!connected && (
                <div className="status-message">Not connected. Select interface and connect.</div>
            )}

            {connected && !selectedBoard && (
                <div className="status-message">Select a board to view fast pins.</div>
            )}

            {error && (
                <div className="error-message">{error}</div>
            )}

            {connected && selectedBoard && (
                <>
                    {/* Quick Actions */}
                    <div className="quick-actions">
                        <button onClick={() => setAllPins(0xFFFFFFFF, 0xFFFFFFFF)} disabled={loading}>
                            All High
                        </button>
                        <button onClick={() => setAllPins(0x00000000, 0xFFFFFFFF)} disabled={loading}>
                            All Low
                        </button>
                        <button onClick={() => setAllPins(0x00000000, 0x00000000)} disabled={loading}>
                            All Hi-Z
                        </button>
                    </div>

                    {/* Pin Grid */}
                    <div className="pin-grid">
                        {Array.from({ length: 32 }, (_, i) => {
                            const pinIndex = 31 - i // Show MSB first
                            const isOutput = pinState ? getBit(pinState.oen, pinIndex) : false
                            const driveValue = pinState ? getBit(pinState.dout, pinIndex) : false
                            const actualValue = pinState ? getBit(pinState.din, pinIndex) : false

                            return (
                                <div key={pinIndex} className={`pin-cell ${getPinClass(pinIndex)}`}>
                                    <div className="pin-number">{128 + pinIndex}</div>
                                    <div className="pin-value">
                                        {isOutput ? (driveValue ? '1' : '0') : 'Z'}
                                    </div>
                                    <div className="pin-actual">
                                        in: {actualValue ? '1' : '0'}
                                    </div>
                                    <div className="pin-controls">
                                        <button
                                            className={`oe-btn ${isOutput ? 'active' : ''}`}
                                            onClick={() => togglePin(pinIndex, 'oen')}
                                            disabled={loading}
                                            title="Toggle Output Enable"
                                        >
                                            OE
                                        </button>
                                        <button
                                            className={`val-btn ${driveValue ? 'active' : ''}`}
                                            onClick={() => togglePin(pinIndex, 'dout')}
                                            disabled={loading || !isOutput}
                                            title="Toggle Drive Value"
                                        >
                                            D
                                        </button>
                                    </div>
                                </div>
                            )
                        })}
                    </div>

                    {/* Register Values */}
                    {pinState && (
                        <div className="register-values">
                            <div className="reg-row">
                                <span className="reg-label">DOUT:</span>
                                <span className="reg-value mono">0x{pinState.dout.toString(16).padStart(8, '0').toUpperCase()}</span>
                            </div>
                            <div className="reg-row">
                                <span className="reg-label">OEN:</span>
                                <span className="reg-value mono">0x{pinState.oen.toString(16).padStart(8, '0').toUpperCase()}</span>
                            </div>
                            <div className="reg-row">
                                <span className="reg-label">DIN:</span>
                                <span className="reg-value mono">0x{pinState.din.toString(16).padStart(8, '0').toUpperCase()}</span>
                            </div>
                        </div>
                    )}
                </>
            )}
        </div>
    )
}
