import { useState } from 'react'
import './FacilityPanel.css'

export default function FacilityPanel() {
    const [valves, setValves] = useState({
        mainWater: false,
        drain: false,
        nitrogen: false
    })

    const [pumps, setPumps] = useState({
        mainFlow: 0, // 0-100%
        boost: false
    })

    const [thermal, setThermal] = useState({
        heater: false,
        cooler: false,
        setpoint: 85
    })

    const toggleValve = (name: keyof typeof valves) => {
        setValves(prev => ({ ...prev, [name]: !prev[name] }))
        // TODO: Send FBC command
        console.log(`Toggled ${name} to ${!valves[name]}`)
    }

    return (
        <div className="facility-panel">
            <h2 className="facility-header">Facility Control</h2>

            <div className="control-grid">

                {/* Fluid Control Section */}
                <div className="control-group">
                    <span className="group-title">Fluid Systems</span>

                    <div className="toggle-list">
                        <Toggle
                            label="Main Water Valve"
                            active={valves.mainWater}
                            onClick={() => toggleValve('mainWater')}
                            color="blue"
                        />
                        <Toggle
                            label="Drain Valve"
                            active={valves.drain}
                            onClick={() => toggleValve('drain')}
                            color="yellow"
                        />
                        <Toggle
                            label="N2 Purge"
                            active={valves.nitrogen}
                            onClick={() => toggleValve('nitrogen')}
                            color="yellow"
                        />
                    </div>

                    <div className="slider-container">
                        <label className="slider-label">Pump Flow Rate ({pumps.mainFlow}%)</label>
                        <input
                            type="range"
                            min="0"
                            max="100"
                            value={pumps.mainFlow}
                            onChange={(e) => setPumps({ ...pumps, mainFlow: parseInt(e.target.value) })}
                            className="range-input"
                        />
                    </div>
                </div>

                {/* Thermal Control Section */}
                <div className="control-group">
                    <span className="group-title">Thermal Systems</span>

                    <div className="temp-control">
                        <span className="temp-label">Target Temp</span>
                        <div className="temp-input-group">
                            <input
                                type="number"
                                value={thermal.setpoint}
                                onChange={(e) => setThermal({ ...thermal, setpoint: parseInt(e.target.value) })}
                                className="temp-input"
                            />
                            <span className="temp-unit">°C</span>
                        </div>
                    </div>

                    <div className="toggle-list">
                        <Toggle
                            label="Heater Core"
                            active={thermal.heater}
                            onClick={() => setThermal(p => ({ ...p, heater: !p.heater }))}
                            color="red"
                        />
                        <Toggle
                            label="Active Cooling"
                            active={thermal.cooler}
                            onClick={() => setThermal(p => ({ ...p, cooler: !p.cooler }))}
                            color="blue"
                        />
                    </div>
                </div>
            </div>

            {/* Emergency Stop */}
            <div className="emergency-section">
                <button
                    className="btn-estop"
                    onClick={() => alert("EMERGENCY STOP TRIGGERED")}
                >
                    EMERGENCY STOP
                </button>
            </div>
        </div>
    )
}

function Toggle({
    label,
    active,
    onClick,
    color = 'green',
}: {
    label: string
    active: boolean
    onClick: () => void
    color?: 'green' | 'red' | 'blue' | 'yellow'
}) {
    return (
        <div className="toggle-item" onClick={onClick}>
            <span className="toggle-label">{label}</span>
            <div className={`toggle-switch ${active ? 'active ' + color : ''}`}>
                <div className="toggle-knob" />
            </div>
        </div>
    )
}
