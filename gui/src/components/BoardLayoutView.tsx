import { useState, useRef, useEffect } from 'react'
import './BoardLayoutView.css'

// ============================================================================
// Board Layout Visualization with Error Overlay
// ============================================================================

// Pin position on board layout (normalized 0-1 coordinates)
interface PinPosition {
    pin: number
    x: number      // 0-1 ratio of board width
    y: number      // 0-1 ratio of board height
    label: string  // Pin name/function
    group: 'bim' | 'fast' | 'power' | 'signal' | 'ground'
    connector?: string  // J3, J4, etc.
}

interface BoardLayoutViewProps {
    errorPins: number[]
    selectedPin?: number
    onPinClick?: (pin: number) => void
    showLabels?: boolean
    highlightMode?: 'errors' | 'all' | 'selected'
    boardImage?: string  // Path to board image/schematic
}

// FBC Controller pin mapping (based on PIN_MAPPING.md)
// These are approximate positions - would be refined with actual board CAD data
const FBC_PIN_POSITIONS: PinPosition[] = [
    // QTH-090 Connector J3 - Left side (BIM pins 0-63)
    ...Array.from({ length: 64 }, (_, i) => ({
        pin: i,
        x: 0.08 + (i % 4) * 0.03,
        y: 0.15 + Math.floor(i / 4) * 0.045,
        label: `BIM${i}`,
        group: 'bim' as const,
        connector: 'J3',
    })),

    // QTH-090 Connector J4 - Right side (BIM pins 64-127)
    ...Array.from({ length: 64 }, (_, i) => ({
        pin: 64 + i,
        x: 0.80 + (i % 4) * 0.03,
        y: 0.15 + Math.floor(i / 4) * 0.045,
        label: `BIM${64 + i}`,
        group: 'bim' as const,
        connector: 'J4',
    })),

    // Fast pins (direct FPGA connection) - Header J8
    ...Array.from({ length: 32 }, (_, i) => ({
        pin: 128 + i,
        x: 0.35 + (i % 8) * 0.04,
        y: 0.75 + Math.floor(i / 8) * 0.05,
        label: `FAST${i}`,
        group: 'fast' as const,
        connector: 'J8',
    })),
]

// Major components on board
const BOARD_COMPONENTS = [
    { id: 'zynq', label: 'Zynq 7020', x: 0.45, y: 0.40, width: 0.12, height: 0.12 },
    { id: 'ddr3', label: 'DDR3', x: 0.45, y: 0.55, width: 0.12, height: 0.05 },
    { id: 'gem', label: 'Ethernet PHY', x: 0.60, y: 0.35, width: 0.08, height: 0.06 },
    { id: 'vicor1', label: 'VICOR 1', x: 0.25, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'vicor2', label: 'VICOR 2', x: 0.35, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'vicor3', label: 'VICOR 3', x: 0.45, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'vicor4', label: 'VICOR 4', x: 0.55, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'vicor5', label: 'VICOR 5', x: 0.65, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'vicor6', label: 'VICOR 6', x: 0.75, y: 0.08, width: 0.06, height: 0.04 },
    { id: 'qspi', label: 'QSPI Flash', x: 0.30, y: 0.40, width: 0.06, height: 0.04 },
    { id: 'sd', label: 'SD Card', x: 0.25, y: 0.55, width: 0.05, height: 0.08 },
    { id: 'eeprom', label: 'EEPROM', x: 0.70, y: 0.50, width: 0.04, height: 0.03 },
    { id: 'xadc', label: 'XADC', x: 0.60, y: 0.48, width: 0.04, height: 0.03 },
]

// Connector outlines
const CONNECTORS = [
    { id: 'J3', label: 'J3 (QTH-090)', x: 0.05, y: 0.12, width: 0.15, height: 0.75 },
    { id: 'J4', label: 'J4 (QTH-090)', x: 0.78, y: 0.12, width: 0.15, height: 0.75 },
    { id: 'J8', label: 'J8 (Fast Pins)', x: 0.32, y: 0.72, width: 0.36, height: 0.20 },
    { id: 'J1', label: 'J1 (JTAG)', x: 0.18, y: 0.30, width: 0.06, height: 0.08 },
    { id: 'RJ45', label: 'Ethernet', x: 0.68, y: 0.28, width: 0.08, height: 0.10 },
]

export default function BoardLayoutView({
    errorPins,
    selectedPin,
    onPinClick,
    showLabels = true,
    highlightMode = 'errors',
}: BoardLayoutViewProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const [dimensions, setDimensions] = useState({ width: 800, height: 600 })
    const [hoveredPin, setHoveredPin] = useState<number | null>(null)
    const [zoom, setZoom] = useState(1)
    const [panOffset, setPanOffset] = useState({ x: 0, y: 0 })

    // Calculate container size
    useEffect(() => {
        const updateSize = () => {
            if (containerRef.current) {
                const rect = containerRef.current.getBoundingClientRect()
                setDimensions({
                    width: Math.max(600, rect.width),
                    height: Math.max(450, rect.height - 60)
                })
            }
        }
        updateSize()
        window.addEventListener('resize', updateSize)
        return () => window.removeEventListener('resize', updateSize)
    }, [])

    // Get pin at position
    const getPinInfo = (pin: number) => FBC_PIN_POSITIONS.find(p => p.pin === pin)

    return (
        <div className="board-layout-view" ref={containerRef}>
            {/* Controls */}
            <div className="layout-controls">
                <div className="zoom-controls">
                    <button onClick={() => setZoom(z => Math.max(0.5, z - 0.25))}>−</button>
                    <span>{Math.round(zoom * 100)}%</span>
                    <button onClick={() => setZoom(z => Math.min(3, z + 0.25))}>+</button>
                    <button onClick={() => { setZoom(1); setPanOffset({ x: 0, y: 0 }) }}>Reset</button>
                </div>
                <div className="view-modes">
                    <span className="mode-label">Show:</span>
                    <span className={`mode-chip ${highlightMode === 'errors' ? 'active' : ''}`}>
                        Errors ({errorPins.length})
                    </span>
                    <span className={`mode-chip ${highlightMode === 'all' ? 'active' : ''}`}>
                        All Pins
                    </span>
                </div>
                {errorPins.length > 0 && (
                    <div className="error-summary">
                        <span className="error-icon">⚠️</span>
                        <span>{errorPins.length} pin{errorPins.length !== 1 ? 's' : ''} with errors</span>
                    </div>
                )}
            </div>

            {/* Board Canvas */}
            <div
                className="board-canvas"
                style={{
                    width: dimensions.width,
                    height: dimensions.height,
                    transform: `scale(${zoom}) translate(${panOffset.x}px, ${panOffset.y}px)`,
                }}
            >
                {/* Board Outline */}
                <svg
                    width={dimensions.width}
                    height={dimensions.height}
                    className="board-svg"
                >
                    {/* Board background */}
                    <rect
                        x="2%"
                        y="2%"
                        width="96%"
                        height="96%"
                        rx="8"
                        className="board-outline"
                    />

                    {/* Connectors */}
                    {CONNECTORS.map(conn => (
                        <g key={conn.id}>
                            <rect
                                x={`${conn.x * 100}%`}
                                y={`${conn.y * 100}%`}
                                width={`${conn.width * 100}%`}
                                height={`${conn.height * 100}%`}
                                rx="4"
                                className="connector-outline"
                            />
                            {showLabels && (
                                <text
                                    x={`${(conn.x + conn.width / 2) * 100}%`}
                                    y={`${conn.y * 100 - 1}%`}
                                    className="connector-label"
                                >
                                    {conn.label}
                                </text>
                            )}
                        </g>
                    ))}

                    {/* Components */}
                    {BOARD_COMPONENTS.map(comp => (
                        <g key={comp.id}>
                            <rect
                                x={`${comp.x * 100}%`}
                                y={`${comp.y * 100}%`}
                                width={`${comp.width * 100}%`}
                                height={`${comp.height * 100}%`}
                                rx="3"
                                className={`component-outline ${comp.id}`}
                            />
                            {showLabels && (
                                <text
                                    x={`${(comp.x + comp.width / 2) * 100}%`}
                                    y={`${(comp.y + comp.height / 2 + 0.01) * 100}%`}
                                    className="component-label"
                                >
                                    {comp.label}
                                </text>
                            )}
                        </g>
                    ))}

                    {/* All pins (faded) when showing all */}
                    {highlightMode === 'all' && FBC_PIN_POSITIONS.map(pin => (
                        <circle
                            key={`bg-${pin.pin}`}
                            cx={`${pin.x * 100}%`}
                            cy={`${pin.y * 100}%`}
                            r="4"
                            className={`pin-dot ${pin.group} ${errorPins.includes(pin.pin) ? 'error' : ''}`}
                            onClick={() => onPinClick?.(pin.pin)}
                            onMouseEnter={() => setHoveredPin(pin.pin)}
                            onMouseLeave={() => setHoveredPin(null)}
                        />
                    ))}

                    {/* Error pins (highlighted) */}
                    {errorPins.map(pinNum => {
                        const pin = getPinInfo(pinNum)
                        if (!pin) return null
                        return (
                            <g key={`error-${pinNum}`} className="error-pin-group">
                                {/* Pulse ring */}
                                <circle
                                    cx={`${pin.x * 100}%`}
                                    cy={`${pin.y * 100}%`}
                                    r="12"
                                    className="error-pulse"
                                />
                                {/* Error dot */}
                                <circle
                                    cx={`${pin.x * 100}%`}
                                    cy={`${pin.y * 100}%`}
                                    r="6"
                                    className="error-dot"
                                    onClick={() => onPinClick?.(pinNum)}
                                    onMouseEnter={() => setHoveredPin(pinNum)}
                                    onMouseLeave={() => setHoveredPin(null)}
                                />
                                {/* Pin number */}
                                {showLabels && (
                                    <text
                                        x={`${pin.x * 100}%`}
                                        y={`${pin.y * 100 - 2}%`}
                                        className="error-pin-label"
                                    >
                                        {pinNum}
                                    </text>
                                )}
                            </g>
                        )
                    })}

                    {/* Selected pin highlight */}
                    {selectedPin !== undefined && (() => {
                        const pin = getPinInfo(selectedPin)
                        if (!pin) return null
                        return (
                            <g className="selected-pin-group">
                                <circle
                                    cx={`${pin.x * 100}%`}
                                    cy={`${pin.y * 100}%`}
                                    r="16"
                                    className="selected-ring"
                                />
                                <line
                                    x1={`${pin.x * 100}%`}
                                    y1={`${pin.y * 100 - 3}%`}
                                    x2={`${pin.x * 100}%`}
                                    y2={`${pin.y * 100 - 8}%`}
                                    className="selected-pointer"
                                />
                            </g>
                        )
                    })()}

                    {/* Trace lines from FPGA to pins (for error pins) */}
                    {errorPins.slice(0, 10).map(pinNum => {
                        const pin = getPinInfo(pinNum)
                        if (!pin) return null
                        const fpga = BOARD_COMPONENTS.find(c => c.id === 'zynq')!
                        return (
                            <line
                                key={`trace-${pinNum}`}
                                x1={`${(fpga.x + fpga.width / 2) * 100}%`}
                                y1={`${(fpga.y + fpga.height / 2) * 100}%`}
                                x2={`${pin.x * 100}%`}
                                y2={`${pin.y * 100}%`}
                                className="error-trace"
                            />
                        )
                    })}
                </svg>

                {/* Tooltip */}
                {hoveredPin !== null && (() => {
                    const pin = getPinInfo(hoveredPin)
                    if (!pin) return null
                    return (
                        <div
                            className="pin-tooltip"
                            style={{
                                left: `${pin.x * 100}%`,
                                top: `${pin.y * 100 + 3}%`,
                            }}
                        >
                            <div className="tooltip-header">
                                Pin {pin.pin} ({pin.label})
                            </div>
                            <div className="tooltip-body">
                                <div>Type: <strong>{pin.group.toUpperCase()}</strong></div>
                                <div>Connector: <strong>{pin.connector}</strong></div>
                                {errorPins.includes(pin.pin) && (
                                    <div className="tooltip-error">⚠️ Error detected</div>
                                )}
                            </div>
                        </div>
                    )
                })()}
            </div>

            {/* Legend */}
            <div className="layout-legend">
                <div className="legend-item">
                    <div className="legend-dot bim" />
                    <span>BIM Pins (0-127)</span>
                </div>
                <div className="legend-item">
                    <div className="legend-dot fast" />
                    <span>Fast Pins (128-159)</span>
                </div>
                <div className="legend-item">
                    <div className="legend-dot error" />
                    <span>Error Detected</span>
                </div>
                <div className="legend-item">
                    <div className="legend-dot selected" />
                    <span>Selected</span>
                </div>
            </div>
        </div>
    )
}
