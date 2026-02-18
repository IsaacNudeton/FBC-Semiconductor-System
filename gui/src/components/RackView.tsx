import { useRef } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text } from '@react-three/drei'
import { useStore } from '../store'
import * as THREE from 'three'

// Color mapping for board states
const stateColors = {
  idle: '#666666',
  running: '#00d26a',
  done: '#4488ff',
  error: '#ff4444',
  unknown: '#333333',
}

// Single board component
function Board({
  position,
  shelf,
  tray,
  slot,
  onBoardClick,
}: {
  position: [number, number, number]
  shelf: number
  tray: 'front' | 'back'
  slot: number
  onBoardClick?: (mac: string) => void
}) {
  const meshRef = useRef<THREE.Mesh>(null)
  const { getBoardAtPosition, selectedBoard, setSelectedBoard } = useStore()
  const board = getBoardAtPosition(shelf, tray, slot)

  const color = board ? stateColors[board.state] || stateColors.unknown : '#222'
  const isSelected = board && selectedBoard === board.mac
  const hasBoard = !!board

  // Pulse animation for running boards
  useFrame((state) => {
    if (meshRef.current && board?.state === 'running') {
      const scale = 1 + Math.sin(state.clock.elapsedTime * 3) * 0.02
      meshRef.current.scale.setScalar(scale)
    }
  })

  const handleClick = () => {
    if (board) {
      setSelectedBoard(board.mac)
      onBoardClick?.(board.mac)
    }
  }

  return (
    <mesh
      ref={meshRef}
      position={position}
      onClick={handleClick}
      onPointerOver={() => (document.body.style.cursor = hasBoard ? 'pointer' : 'default')}
      onPointerOut={() => (document.body.style.cursor = 'default')}
    >
      <boxGeometry args={[0.8, 0.15, 1.8]} />
      <meshStandardMaterial
        color={color}
        emissive={isSelected ? '#ffffff' : color}
        emissiveIntensity={isSelected ? 0.3 : board?.state === 'running' ? 0.2 : 0}
        metalness={0.3}
        roughness={0.7}
      />
    </mesh>
  )
}

// Tray component (holds 4 boards)
function Tray({
  position,
  shelf,
  tray,
  boardsPerTray,
  onBoardClick,
}: {
  position: [number, number, number]
  shelf: number
  tray: 'front' | 'back'
  boardsPerTray: number
  onBoardClick?: (mac: string) => void
}) {
  return (
    <group position={position}>
      {/* Tray base */}
      <mesh position={[0, -0.1, 0]}>
        <boxGeometry args={[4, 0.05, 2.2]} />
        <meshStandardMaterial color="#1a1a1a" metalness={0.5} roughness={0.5} />
      </mesh>

      {/* Tray rails */}
      <mesh position={[-2.05, 0, 0]}>
        <boxGeometry args={[0.1, 0.3, 2.2]} />
        <meshStandardMaterial color="#333" metalness={0.6} roughness={0.4} />
      </mesh>
      <mesh position={[2.05, 0, 0]}>
        <boxGeometry args={[0.1, 0.3, 2.2]} />
        <meshStandardMaterial color="#333" metalness={0.6} roughness={0.4} />
      </mesh>

      {/* Boards on tray */}
      {Array.from({ length: boardsPerTray }).map((_, i) => (
        <Board
          key={i}
          position={[-1.5 + i * 1, 0.1, 0]}
          shelf={shelf}
          tray={tray}
          slot={i + 1}
          onBoardClick={onBoardClick}
        />
      ))}
    </group>
  )
}

// Shelf component (front and back trays)
function Shelf({
  position,
  shelfNum,
  boardsPerTray,
  dualTray,
  onBoardClick,
}: {
  position: [number, number, number]
  shelfNum: number
  boardsPerTray: number
  dualTray: boolean
  onBoardClick?: (mac: string) => void
}) {
  return (
    <group position={position}>
      {/* Shelf label */}
      <Text
        position={[-3.2, 0, 0]}
        fontSize={0.3}
        color="#666"
        anchorX="right"
      >
        {`S${shelfNum}`}
      </Text>
      {/* Front tray */}
      <Tray
        position={[0, 0, dualTray ? 1.5 : 0]}
        shelf={shelfNum}
        tray="front"
        boardsPerTray={boardsPerTray}
        onBoardClick={onBoardClick}
      />

      {/* Back tray */}
      {dualTray && (
        <Tray
          position={[0, 0, -1.5]}
          shelf={shelfNum}
          tray="back"
          boardsPerTray={boardsPerTray}
          onBoardClick={onBoardClick}
        />
      )}

      {/* Shelf divider */}
      <mesh position={[0, -0.2, 0]}>
        <boxGeometry args={[4.5, 0.02, dualTray ? 5 : 2.5]} />
        <meshStandardMaterial color="#2a2a2a" metalness={0.4} roughness={0.6} />
      </mesh>
    </group>
  )
}

// Main rack component
interface RackViewProps {
  onBoardClick?: (mac: string) => void
}

export default function RackView({ onBoardClick }: RackViewProps) {
  const { rackConfig } = useStore()
  const { shelves, boards_per_tray, dual_tray } = rackConfig

  const shelfHeight = 0.8 // Height between shelves
  const totalHeight = shelves * shelfHeight

  return (
    <group>
      {/* Ground grid */}
      <gridHelper args={[20, 20, '#222', '#181818']} position={[0, -0.3, 0]} />

      {/* Rack frame */}
      {/* Left side */}
      <mesh position={[-2.5, totalHeight / 2, 0]}>
        <boxGeometry args={[0.1, totalHeight + 0.5, dual_tray ? 5.5 : 3]} />
        <meshStandardMaterial color="#1a1a1a" metalness={0.6} roughness={0.4} />
      </mesh>
      {/* Right side */}
      <mesh position={[2.5, totalHeight / 2, 0]}>
        <boxGeometry args={[0.1, totalHeight + 0.5, dual_tray ? 5.5 : 3]} />
        <meshStandardMaterial color="#1a1a1a" metalness={0.6} roughness={0.4} />
      </mesh>
      {/* Back panel */}
      <mesh position={[0, totalHeight / 2, dual_tray ? -2.8 : -1.6]}>
        <boxGeometry args={[5.1, totalHeight + 0.5, 0.1]} />
        <meshStandardMaterial color="#111" metalness={0.5} roughness={0.5} />
      </mesh>
      {/* Top */}
      <mesh position={[0, totalHeight + 0.3, 0]}>
        <boxGeometry args={[5.1, 0.1, dual_tray ? 5.5 : 3]} />
        <meshStandardMaterial color="#1a1a1a" metalness={0.6} roughness={0.4} />
      </mesh>
      {/* Base */}
      <mesh position={[0, -0.1, 0]}>
        <boxGeometry args={[5.1, 0.2, dual_tray ? 5.5 : 3]} />
        <meshStandardMaterial color="#1a1a1a" metalness={0.6} roughness={0.4} />
      </mesh>

      {/* Shelves */}
      {Array.from({ length: shelves }).map((_, i) => (
        <Shelf
          key={i}
          position={[0, i * shelfHeight + 0.3, 0]}
          shelfNum={i + 1}
          boardsPerTray={boards_per_tray}
          dualTray={dual_tray}
          onBoardClick={onBoardClick}
        />
      ))}
    </group>
  )
}
