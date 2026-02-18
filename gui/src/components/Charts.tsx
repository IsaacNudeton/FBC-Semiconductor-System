import { useRef, useEffect, useMemo } from 'react'
import './Charts.css'

// ============================================================================
// Reusable Chart Components for FBC GUI
// ============================================================================

interface DataPoint {
    timestamp: number
    value: number
    label?: string
}

interface LineChartProps {
    data: DataPoint[]
    width?: number
    height?: number
    color?: string
    fillColor?: string
    showGrid?: boolean
    showAxis?: boolean
    minValue?: number
    maxValue?: number
    formatValue?: (v: number) => string
    formatTime?: (t: number) => string
    title?: string
    unit?: string
}

/**
 * Real-time line chart using Canvas for performance
 */
export function LineChart({
    data,
    width = 400,
    height = 150,
    color = '#4ade80',
    fillColor = 'rgba(74, 222, 128, 0.15)',
    showGrid = true,
    showAxis = true,
    minValue,
    maxValue,
    formatValue = (v) => v.toFixed(1),
    formatTime = (t) => new Date(t).toLocaleTimeString(),
    title,
    unit = '',
}: LineChartProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null)

    const { min, max, range } = useMemo(() => {
        if (data.length === 0) return { min: 0, max: 100, range: 100 }
        const values = data.map(d => d.value)
        const dataMin = Math.min(...values)
        const dataMax = Math.max(...values)
        const padding = (dataMax - dataMin) * 0.1 || 10
        const min = minValue !== undefined ? minValue : dataMin - padding
        const max = maxValue !== undefined ? maxValue : dataMax + padding
        return { min, max, range: max - min }
    }, [data, minValue, maxValue])

    useEffect(() => {
        const canvas = canvasRef.current
        if (!canvas) return
        const ctx = canvas.getContext('2d')
        if (!ctx) return

        const dpr = window.devicePixelRatio || 1
        canvas.width = width * dpr
        canvas.height = height * dpr
        ctx.scale(dpr, dpr)

        // Clear
        ctx.clearRect(0, 0, width, height)

        const padding = { left: showAxis ? 50 : 10, right: 10, top: 20, bottom: showAxis ? 25 : 10 }
        const chartWidth = width - padding.left - padding.right
        const chartHeight = height - padding.top - padding.bottom

        // Grid lines
        if (showGrid) {
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)'
            ctx.lineWidth = 1

            // Horizontal grid
            for (let i = 0; i <= 4; i++) {
                const y = padding.top + (chartHeight / 4) * i
                ctx.beginPath()
                ctx.moveTo(padding.left, y)
                ctx.lineTo(width - padding.right, y)
                ctx.stroke()
            }

            // Vertical grid
            for (let i = 0; i <= 5; i++) {
                const x = padding.left + (chartWidth / 5) * i
                ctx.beginPath()
                ctx.moveTo(x, padding.top)
                ctx.lineTo(x, height - padding.bottom)
                ctx.stroke()
            }
        }

        // Axis labels
        if (showAxis) {
            ctx.fillStyle = 'rgba(255, 255, 255, 0.6)'
            ctx.font = '10px Inter, sans-serif'
            ctx.textAlign = 'right'

            // Y-axis labels
            for (let i = 0; i <= 4; i++) {
                const y = padding.top + (chartHeight / 4) * i
                const value = max - (range / 4) * i
                ctx.fillText(formatValue(value), padding.left - 5, y + 3)
            }

            // X-axis labels (time)
            if (data.length >= 2) {
                ctx.textAlign = 'center'
                const first = data[0].timestamp
                const last = data[data.length - 1].timestamp

                ctx.fillText(formatTime(first), padding.left, height - 5)
                ctx.fillText(formatTime(last), width - padding.right, height - 5)
            }
        }

        // Title
        if (title) {
            ctx.fillStyle = 'rgba(255, 255, 255, 0.8)'
            ctx.font = '12px Inter, sans-serif'
            ctx.textAlign = 'left'
            ctx.fillText(`${title}${unit ? ` (${unit})` : ''}`, padding.left, 12)
        }

        // Plot data
        if (data.length < 2) {
            ctx.fillStyle = 'rgba(255, 255, 255, 0.4)'
            ctx.font = '12px Inter, sans-serif'
            ctx.textAlign = 'center'
            ctx.fillText('Waiting for data...', width / 2, height / 2)
            return
        }

        const timeRange = data[data.length - 1].timestamp - data[0].timestamp || 1

        const getX = (t: number) => padding.left + ((t - data[0].timestamp) / timeRange) * chartWidth
        const getY = (v: number) => padding.top + ((max - v) / range) * chartHeight

        // Fill area
        ctx.beginPath()
        ctx.moveTo(getX(data[0].timestamp), height - padding.bottom)
        data.forEach((d) => ctx.lineTo(getX(d.timestamp), getY(d.value)))
        ctx.lineTo(getX(data[data.length - 1].timestamp), height - padding.bottom)
        ctx.closePath()
        ctx.fillStyle = fillColor
        ctx.fill()

        // Line
        ctx.beginPath()
        ctx.moveTo(getX(data[0].timestamp), getY(data[0].value))
        data.forEach((d, i) => {
            if (i === 0) return
            ctx.lineTo(getX(d.timestamp), getY(d.value))
        })
        ctx.strokeStyle = color
        ctx.lineWidth = 2
        ctx.lineCap = 'round'
        ctx.lineJoin = 'round'
        ctx.stroke()

        // Current value indicator
        const latest = data[data.length - 1]
        const lx = getX(latest.timestamp)
        const ly = getY(latest.value)

        // Glow
        ctx.beginPath()
        ctx.arc(lx, ly, 6, 0, Math.PI * 2)
        ctx.fillStyle = fillColor
        ctx.fill()

        // Dot
        ctx.beginPath()
        ctx.arc(lx, ly, 4, 0, Math.PI * 2)
        ctx.fillStyle = color
        ctx.fill()

    }, [data, width, height, color, fillColor, showGrid, showAxis, min, max, range, formatValue, formatTime, title, unit])

    return (
        <div className="line-chart">
            <canvas
                ref={canvasRef}
                style={{ width: `${width}px`, height: `${height}px` }}
            />
        </div>
    )
}

// ============================================================================
// Vector Pattern Waveform Display
// ============================================================================

interface WaveformChannel {
    name: string
    data: number[]  // 0 or 1 per cycle
    type: 'output' | 'input' | 'bidirectional'
    expected?: number[]
    actual?: number[]
}

interface WaveformViewerProps {
    channels: WaveformChannel[]
    startCycle?: number
    cyclesVisible?: number
    highlightErrors?: boolean
    onCycleSelect?: (cycle: number) => void
    selectedCycle?: number
}

export function WaveformViewer({
    channels,
    startCycle = 0,
    cyclesVisible = 32,
    highlightErrors = true,
    onCycleSelect,
    selectedCycle,
}: WaveformViewerProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null)
    const containerRef = useRef<HTMLDivElement>(null)

    const width = 800
    const channelHeight = 30
    const labelWidth = 100
    const height = channels.length * channelHeight + 40

    useEffect(() => {
        const canvas = canvasRef.current
        if (!canvas) return
        const ctx = canvas.getContext('2d')
        if (!ctx) return

        const dpr = window.devicePixelRatio || 1
        canvas.width = width * dpr
        canvas.height = height * dpr
        ctx.scale(dpr, dpr)

        ctx.clearRect(0, 0, width, height)

        const waveWidth = width - labelWidth - 20
        const cycleWidth = waveWidth / cyclesVisible

        // Header (cycle numbers)
        ctx.fillStyle = 'rgba(255, 255, 255, 0.5)'
        ctx.font = '9px monospace'
        ctx.textAlign = 'center'
        for (let i = 0; i < cyclesVisible; i += 4) {
            const cycle = startCycle + i
            const x = labelWidth + i * cycleWidth + cycleWidth / 2
            ctx.fillText(cycle.toString(), x, 12)
        }

        // Channels
        channels.forEach((channel, idx) => {
            const y = 20 + idx * channelHeight
            const waveY = y + channelHeight / 2

            // Label
            ctx.fillStyle = 'rgba(255, 255, 255, 0.8)'
            ctx.font = '11px Inter, sans-serif'
            ctx.textAlign = 'right'
            ctx.fillText(channel.name, labelWidth - 10, waveY + 4)

            // Type indicator
            const typeColors = {
                output: '#4ade80',
                input: '#60a5fa',
                bidirectional: '#c084fc',
            }
            ctx.fillStyle = typeColors[channel.type]
            ctx.fillRect(2, y + 8, 4, channelHeight - 16)

            // Waveform
            ctx.strokeStyle = typeColors[channel.type]
            ctx.lineWidth = 2
            ctx.beginPath()

            for (let i = 0; i < cyclesVisible && (startCycle + i) < channel.data.length; i++) {
                const cycle = startCycle + i
                const value = channel.data[cycle]
                const x = labelWidth + i * cycleWidth
                const highY = waveY - 8
                const lowY = waveY + 8

                if (i === 0) {
                    ctx.moveTo(x, value ? highY : lowY)
                } else {
                    const prevValue = channel.data[cycle - 1]
                    if (prevValue !== value) {
                        // Transition
                        ctx.lineTo(x, prevValue ? highY : lowY)
                        ctx.lineTo(x, value ? highY : lowY)
                    }
                }
                ctx.lineTo(x + cycleWidth, value ? highY : lowY)
            }
            ctx.stroke()

            // Error highlighting
            if (highlightErrors && channel.expected && channel.actual) {
                for (let i = 0; i < cyclesVisible && (startCycle + i) < channel.expected.length; i++) {
                    const cycle = startCycle + i
                    if (channel.expected[cycle] !== channel.actual[cycle]) {
                        const x = labelWidth + i * cycleWidth
                        ctx.fillStyle = 'rgba(239, 68, 68, 0.3)'
                        ctx.fillRect(x, y + 2, cycleWidth, channelHeight - 4)
                    }
                }
            }
        })

        // Selected cycle highlight
        if (selectedCycle !== undefined && selectedCycle >= startCycle && selectedCycle < startCycle + cyclesVisible) {
            const x = labelWidth + (selectedCycle - startCycle) * cycleWidth
            ctx.fillStyle = 'rgba(255, 255, 255, 0.1)'
            ctx.fillRect(x, 20, cycleWidth, height - 20)
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)'
            ctx.lineWidth = 1
            ctx.strokeRect(x, 20, cycleWidth, height - 20)
        }

    }, [channels, startCycle, cyclesVisible, highlightErrors, selectedCycle, height])

    const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (!onCycleSelect) return
        const rect = canvasRef.current?.getBoundingClientRect()
        if (!rect) return
        const x = e.clientX - rect.left
        const waveWidth = width - labelWidth - 20
        const cycleWidth = waveWidth / cyclesVisible
        if (x < labelWidth) return
        const cycle = Math.floor((x - labelWidth) / cycleWidth) + startCycle
        onCycleSelect(cycle)
    }

    return (
        <div className="waveform-viewer" ref={containerRef}>
            <canvas
                ref={canvasRef}
                style={{ width: `${width}px`, height: `${height}px` }}
                onClick={handleClick}
            />
        </div>
    )
}

// ============================================================================
// Error Map / Pin Grid Visualization
// ============================================================================

interface PinGridProps {
    pinCount: number
    errorMask: bigint | number[]
    onPinClick?: (pin: number) => void
    selectedPin?: number
    layout?: '16x10' | '8x20' | '4x40'
}

export function PinGrid({
    pinCount,
    errorMask,
    onPinClick,
    selectedPin,
    layout = '16x10',
}: PinGridProps) {
    const [cols] = layout.split('x').map(Number)

    const hasError = (pin: number): boolean => {
        if (Array.isArray(errorMask)) {
            return errorMask.includes(pin)
        }
        return (BigInt(errorMask) & (BigInt(1) << BigInt(pin))) !== BigInt(0)
    }

    const getPinType = (pin: number): string => {
        if (pin < 128) return 'bim'  // BIM pins (through quad board)
        return 'fast'  // Fast pins (direct FPGA)
    }

    return (
        <div className="pin-grid" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
            {Array.from({ length: pinCount }, (_, pin) => (
                <div
                    key={pin}
                    className={`pin-cell 
            ${hasError(pin) ? 'error' : ''} 
            ${selectedPin === pin ? 'selected' : ''}
            ${getPinType(pin)}`}
                    onClick={() => onPinClick?.(pin)}
                    title={`Pin ${pin} (${getPinType(pin).toUpperCase()})`}
                >
                    {pin}
                </div>
            ))}
        </div>
    )
}

// ============================================================================
// Multi-Series Chart (for comparing multiple values)
// ============================================================================

interface Series {
    name: string
    data: DataPoint[]
    color: string
}

interface MultiSeriesChartProps {
    series: Series[]
    width?: number
    height?: number
    showLegend?: boolean
    title?: string
}

export function MultiSeriesChart({
    series,
    width = 400,
    height = 200,
    showLegend = true,
    title,
}: MultiSeriesChartProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null)

    const { min, max, timeMin, timeMax } = useMemo(() => {
        if (series.length === 0) return { min: 0, max: 100, timeMin: 0, timeMax: 1 }

        let allValues: number[] = []
        let allTimes: number[] = []
        series.forEach(s => {
            s.data.forEach(d => {
                allValues.push(d.value)
                allTimes.push(d.timestamp)
            })
        })

        const dataMin = Math.min(...allValues)
        const dataMax = Math.max(...allValues)
        const padding = (dataMax - dataMin) * 0.1 || 10

        return {
            min: dataMin - padding,
            max: dataMax + padding,
            timeMin: Math.min(...allTimes),
            timeMax: Math.max(...allTimes),
        }
    }, [series])

    useEffect(() => {
        const canvas = canvasRef.current
        if (!canvas) return
        const ctx = canvas.getContext('2d')
        if (!ctx) return

        const dpr = window.devicePixelRatio || 1
        canvas.width = width * dpr
        canvas.height = height * dpr
        ctx.scale(dpr, dpr)

        ctx.clearRect(0, 0, width, height)

        const padding = { left: 50, right: 10, top: 30, bottom: 25 }
        const chartWidth = width - padding.left - padding.right
        const chartHeight = height - padding.top - padding.bottom
        const range = max - min
        const timeRange = timeMax - timeMin || 1

        // Title
        if (title) {
            ctx.fillStyle = 'rgba(255, 255, 255, 0.8)'
            ctx.font = '12px Inter, sans-serif'
            ctx.textAlign = 'left'
            ctx.fillText(title, padding.left, 15)
        }

        // Grid
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)'
        ctx.lineWidth = 1
        for (let i = 0; i <= 4; i++) {
            const y = padding.top + (chartHeight / 4) * i
            ctx.beginPath()
            ctx.moveTo(padding.left, y)
            ctx.lineTo(width - padding.right, y)
            ctx.stroke()
        }

        // Y-axis labels
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)'
        ctx.font = '10px Inter, sans-serif'
        ctx.textAlign = 'right'
        for (let i = 0; i <= 4; i++) {
            const y = padding.top + (chartHeight / 4) * i
            const value = max - (range / 4) * i
            ctx.fillText(value.toFixed(0), padding.left - 5, y + 3)
        }

        // Plot each series
        series.forEach(s => {
            if (s.data.length < 2) return

            const getX = (t: number) => padding.left + ((t - timeMin) / timeRange) * chartWidth
            const getY = (v: number) => padding.top + ((max - v) / range) * chartHeight

            ctx.beginPath()
            ctx.moveTo(getX(s.data[0].timestamp), getY(s.data[0].value))
            s.data.forEach((d, i) => {
                if (i === 0) return
                ctx.lineTo(getX(d.timestamp), getY(d.value))
            })
            ctx.strokeStyle = s.color
            ctx.lineWidth = 2
            ctx.stroke()
        })

        // Legend
        if (showLegend && series.length > 0) {
            const legendY = height - 10
            let legendX = padding.left
            series.forEach(s => {
                ctx.fillStyle = s.color
                ctx.fillRect(legendX, legendY - 8, 12, 12)
                ctx.fillStyle = 'rgba(255, 255, 255, 0.7)'
                ctx.font = '10px Inter, sans-serif'
                ctx.textAlign = 'left'
                ctx.fillText(s.name, legendX + 16, legendY + 2)
                legendX += ctx.measureText(s.name).width + 30
            })
        }

    }, [series, width, height, min, max, timeMin, timeMax, title, showLegend])

    return (
        <div className="multi-series-chart">
            <canvas
                ref={canvasRef}
                style={{ width: `${width}px`, height: `${height}px` }}
            />
        </div>
    )
}
