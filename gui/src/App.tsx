import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import RackView2D from './components/RackView2D'
import Sidebar, { ViewType } from './components/Sidebar'
import FacilityPanel from './components/FacilityPanel'
import BoardDetailPanel from './components/BoardDetailPanel'
import AnalogMonitorPanel from './components/AnalogMonitorPanel'
import PowerControlPanel from './components/PowerControlPanel'
import EepromPanel from './components/EepromPanel'
import VectorEnginePanel from './components/VectorEnginePanel'
import DeviceConfigPanel from './components/DeviceConfigPanel'
import TestPlanEditor from './components/TestPlanEditor'
import { FirmwareUpdatePanel } from './components/FirmwareUpdatePanel'
import Terminal from './components/Terminal'
import Toolbar from './components/Toolbar'
import { useStore } from './store'
import './styles/app.css'

function App() {
  const { connected, setBoards, selectedBoard, setSelectedBoard } = useStore()
  const [activeView, setActiveView] = useState<ViewType>('overview')
  const [terminalOpen, setTerminalOpen] = useState(true)

  // Poll board status when connected
  useEffect(() => {
    if (!connected) return

    const interval = setInterval(async () => {
      try {
        const discovered = await invoke<any[]>('discover_boards')
        setBoards(discovered)
      } catch (e) {
        console.error('Discovery failed:', e)
      }
    }, 5000)

    return () => clearInterval(interval)
  }, [connected])

  // Auto-switch to board-detail when a board is selected
  useEffect(() => {
    if (selectedBoard && activeView === 'overview') {
      setActiveView('board-detail')
    }
  }, [selectedBoard])

  // Handle board click from rack view
  const handleBoardClick = (mac: string) => {
    setSelectedBoard(mac)
    setActiveView('board-detail')
  }

  const renderMainContent = () => {
    switch (activeView) {
      case 'overview':
        return (
          <RackView2D
            onBoardSelect={handleBoardClick}
            onSlotSelect={(shelf, tray, slot) => console.log('Slot selected:', shelf, tray, slot)}
          />
        )

      case 'board-detail':
        return <BoardDetailPanel />

      case 'analog':
        return <AnalogMonitorPanel />

      case 'power':
        return <PowerControlPanel />

      case 'vectors':
        return <VectorEnginePanel />

      case 'config':
        return <DeviceConfigPanel />

      case 'eeprom':
        return <EepromPanel />

      case 'testplan':
        return <TestPlanEditor />

      case 'facility':
        return <FacilityPanel />

      case 'firmware':
        return <FirmwareUpdatePanel />

      default:
        return null
    }
  }

  return (
    <div className="app">
      <Toolbar />

      <div className="main-layout">
        <Sidebar activeView={activeView} onViewChange={setActiveView} />

        <div className="content-area">
          {renderMainContent()}
        </div>
      </div>

      {/* Terminal (bottom) */}
      <Terminal
        isOpen={terminalOpen}
        onToggle={() => setTerminalOpen(!terminalOpen)}
      />
    </div>
  )
}

export default App
