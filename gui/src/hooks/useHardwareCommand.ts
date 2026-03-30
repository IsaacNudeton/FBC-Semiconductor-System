import { invoke } from '@tauri-apps/api/core'
import { useStore } from '../store'

/**
 * Dual-mode command dispatch hook.
 *
 * Abstracts FBC (raw Ethernet, MAC-addressed) vs Sonoma (SSH, IP-addressed)
 * control. Panels call `exec()` with both command names; the hook picks
 * the right one based on `controlMode` and passes the correct board identifier.
 */
export function useHardwareCommand() {
  const controlMode = useStore(s => s.controlMode)
  const selectedBoard = useStore(s => s.selectedBoard)
  const selectedSonomaBoard = useStore(s => s.selectedSonomaBoard)

  /** Whether any board is selected in the current mode */
  const hasBoard = controlMode === 'fbc' ? !!selectedBoard : !!selectedSonomaBoard

  /** The active board identifier (MAC for FBC, IP for Sonoma) */
  const boardId = controlMode === 'fbc' ? selectedBoard : selectedSonomaBoard

  /**
   * Execute a hardware command, dispatching to the correct backend.
   *
   * @param fbcCommand  - Tauri command name for FBC protocol (uses `mac` param)
   * @param sonomaCommand - Tauri command name for Sonoma SSH (uses `ip` param)
   * @param args - Additional arguments (merged with board identifier)
   */
  async function exec<T>(
    fbcCommand: string,
    sonomaCommand: string,
    args: Record<string, unknown> = {},
  ): Promise<T> {
    if (controlMode === 'fbc') {
      if (!selectedBoard) throw new Error('No FBC board selected')
      return invoke<T>(fbcCommand, { mac: selectedBoard, ...args })
    } else {
      if (!selectedSonomaBoard) throw new Error('No Sonoma board selected')
      return invoke<T>(sonomaCommand, { ip: selectedSonomaBoard, ...args })
    }
  }

  return { exec, controlMode, hasBoard, boardId }
}
