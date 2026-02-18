import { useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { useStore, LiveBoardState } from '../store'

// Event payloads from backend (match Rust BoardEvent variants)
interface BoardConnectedEvent {
  Connected: { mac: string; position: { shelf: number; tray: string; slot: number } | null }
}

interface BoardDisconnectedEvent {
  Disconnected: { mac: string }
}

interface BoardStateChangedEvent {
  StateChanged: { mac: string; old_state: string; new_state: string }
}

interface BoardErrorEvent {
  Error: { mac: string; error_count: number }
}

interface BoardHeartbeatEvent {
  Heartbeat: { mac: string; state: LiveBoardState }
}

type BackendBoardEvent =
  | BoardConnectedEvent
  | BoardDisconnectedEvent
  | BoardStateChangedEvent
  | BoardErrorEvent
  | BoardHeartbeatEvent

/**
 * Hook that sets up realtime board monitoring.
 *
 * Automatically:
 * - Fetches initial board states on mount
 * - Subscribes to board events (connect, disconnect, state changes, heartbeats)
 * - Updates the store with live board data
 *
 * @param enabled Whether to enable monitoring (default: true)
 */
export function useRealtimeBoards(enabled = true) {
  const { setLiveBoards, updateLiveBoard, removeLiveBoard, connected } = useStore()

  // Fetch initial boards
  const fetchBoards = useCallback(async () => {
    try {
      const boards = await invoke<LiveBoardState[]>('get_live_boards')
      setLiveBoards(boards)
    } catch (err) {
      console.error('Failed to fetch live boards:', err)
    }
  }, [setLiveBoards])

  useEffect(() => {
    if (!enabled || !connected) return

    // Fetch initial state
    fetchBoards()

    // Set up event listeners
    const unlisteners: UnlistenFn[] = []

    const setupListeners = async () => {
      // Board connected
      unlisteners.push(
        await listen<BackendBoardEvent>('board:connected', (event) => {
          console.log('Board connected:', event.payload)
          // Fetch updated board list
          fetchBoards()
        })
      )

      // Board disconnected
      unlisteners.push(
        await listen<BackendBoardEvent>('board:disconnected', (event) => {
          const payload = event.payload as BoardDisconnectedEvent
          if ('Disconnected' in payload) {
            removeLiveBoard(payload.Disconnected.mac)
          }
        })
      )

      // Board state changed
      unlisteners.push(
        await listen<BackendBoardEvent>('board:state-changed', (event) => {
          console.log('Board state changed:', event.payload)
          // Fetch updated state for this board
          fetchBoards()
        })
      )

      // Board error
      unlisteners.push(
        await listen<BackendBoardEvent>('board:error', (event) => {
          console.log('Board error:', event.payload)
        })
      )

      // Board heartbeat (most frequent - updates live state)
      unlisteners.push(
        await listen<BackendBoardEvent>('board:heartbeat', (event) => {
          const payload = event.payload as BoardHeartbeatEvent
          if ('Heartbeat' in payload) {
            updateLiveBoard(payload.Heartbeat.state)
          }
        })
      )
    }

    setupListeners()

    // Cleanup
    return () => {
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [enabled, connected, fetchBoards, updateLiveBoard, removeLiveBoard])

  return { fetchBoards }
}

/**
 * Format run time from milliseconds to HH:MM:SS
 */
export function formatRunTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

/**
 * Format uptime from milliseconds to human readable
 */
export function formatUptime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  if (totalSeconds < 60) return `${totalSeconds}s`
  if (totalSeconds < 3600) return `${Math.floor(totalSeconds / 60)}m`
  if (totalSeconds < 86400) return `${Math.floor(totalSeconds / 3600)}h`
  return `${Math.floor(totalSeconds / 86400)}d`
}
