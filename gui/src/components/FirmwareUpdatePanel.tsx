import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';
import { useStore } from '../store';
import './FirmwareUpdatePanel.css';

interface FbcFirmwareInfo {
  version: string;
  build_date: string;
  board_serial: number;
  hw_revision: number;
  sd_present: boolean;
  update_in_progress: boolean;
}

interface BoardFirmwareState {
  mac: string;
  info: FbcFirmwareInfo | null;
  loading: boolean;
  error: string | null;
  updating: boolean;
  progress: number;
  progressMessage: string;
}

export function FirmwareUpdatePanel() {
  const { liveBoards, selectedBoard, setSelectedBoard } = useStore();
  const [firmwarePath, setFirmwarePath] = useState<string>('');
  const [firmwareData, setFirmwareData] = useState<Uint8Array | null>(null);
  const [boardStates, setBoardStates] = useState<Map<string, BoardFirmwareState>>(new Map());
  const [log, setLog] = useState<string[]>([]);

  const addLog = (msg: string) => {
    setLog(prev => [...prev.slice(-100), `[${new Date().toLocaleTimeString()}] ${msg}`]);
  };

  // Convert live boards to array
  const boards = Array.from(liveBoards.values()).filter(b => b.online);

  // Fetch firmware info for all boards on mount
  useEffect(() => {
    boards.forEach(board => {
      if (!boardStates.has(board.mac)) {
        fetchFirmwareInfo(board.mac);
      }
    });
  }, [liveBoards]);

  // Listen for progress events
  useEffect(() => {
    const unlisten = listen<{ stage: string; percent: number; message: string }>(
      'firmware:progress',
      (event) => {
        const { percent, message } = event.payload;
        // Update the board that's currently updating
        setBoardStates(prev => {
          const next = new Map(prev);
          for (const [mac, state] of next) {
            if (state.updating) {
              next.set(mac, { ...state, progress: percent, progressMessage: message });
            }
          }
          return next;
        });
        addLog(message);
      }
    );

    return () => { unlisten.then(f => f()); };
  }, []);

  const fetchFirmwareInfo = async (mac: string) => {
    setBoardStates(prev => {
      const next = new Map(prev);
      next.set(mac, {
        mac,
        info: null,
        loading: true,
        error: null,
        updating: false,
        progress: 0,
        progressMessage: '',
      });
      return next;
    });

    try {
      const info: FbcFirmwareInfo = await invoke('get_firmware_info', { mac });
      setBoardStates(prev => {
        const next = new Map(prev);
        const existing = next.get(mac);
        next.set(mac, { ...existing!, info, loading: false });
        return next;
      });
      addLog(`${mac}: v${info.version} (${info.build_date})`);
    } catch (e) {
      setBoardStates(prev => {
        const next = new Map(prev);
        const existing = next.get(mac);
        next.set(mac, { ...existing!, loading: false, error: String(e) });
        return next;
      });
      addLog(`${mac}: Failed to get info - ${e}`);
    }
  };

  const selectFirmware = async () => {
    const file = await open({
      filters: [{ name: 'Boot Image', extensions: ['BIN', 'bin'] }],
      multiple: false,
    });
    if (file) {
      setFirmwarePath(file as string);
      addLog(`Selected: ${file}`);

      // Read file contents
      try {
        const data = await readFile(file as string);
        setFirmwareData(data);
        addLog(`Loaded ${data.length} bytes`);
      } catch (e) {
        addLog(`Failed to read file: ${e}`);
        setFirmwareData(null);
      }
    }
  };

  const updateBoard = async (mac: string) => {
    if (!firmwareData) {
      addLog('ERROR: No firmware loaded');
      return;
    }

    const state = boardStates.get(mac);
    if (!state?.info?.sd_present) {
      addLog(`${mac}: No SD card - cannot update`);
      return;
    }

    setBoardStates(prev => {
      const next = new Map(prev);
      const existing = next.get(mac);
      next.set(mac, { ...existing!, updating: true, progress: 0, progressMessage: 'Starting...' });
      return next;
    });

    addLog(`${mac}: Starting firmware update...`);

    try {
      const result: string = await invoke('update_firmware_fbc', {
        mac,
        firmwareData: Array.from(firmwareData),
      });
      addLog(`${mac}: ${result}`);

      setBoardStates(prev => {
        const next = new Map(prev);
        const existing = next.get(mac);
        next.set(mac, { ...existing!, updating: false, progress: 100, progressMessage: 'Complete!' });
        return next;
      });
    } catch (e) {
      addLog(`${mac}: FAILED - ${e}`);
      setBoardStates(prev => {
        const next = new Map(prev);
        const existing = next.get(mac);
        next.set(mac, { ...existing!, updating: false, error: String(e), progressMessage: 'Failed' });
        return next;
      });
    }
  };

  const refreshAll = () => {
    boards.forEach(board => fetchFirmwareInfo(board.mac));
  };

  return (
    <div className="firmware-update-panel">
      <div className="panel-header">
        <h2>Firmware Update</h2>
        <button className="refresh-btn" onClick={refreshAll}>Refresh All</button>
      </div>

      {/* Firmware Selection */}
      <div className="firmware-section">
        <h3>1. Select Firmware File</h3>
        <div className="firmware-file">
          <input
            type="text"
            value={firmwarePath}
            placeholder="No file selected..."
            readOnly
          />
          <button onClick={selectFirmware}>Browse...</button>
        </div>
        {firmwareData && (
          <div className="firmware-info">
            <span className="size">{(firmwareData.length / 1024).toFixed(1)} KB</span>
            <span className="ready">Ready to upload</span>
          </div>
        )}
      </div>

      {/* Board List */}
      <div className="firmware-section">
        <h3>2. Select Board & Update</h3>
        <div className="board-grid">
          {boards.length === 0 ? (
            <div className="no-boards">
              No boards online. Connect hardware and wait for detection.
            </div>
          ) : (
            boards.map(board => {
              const state = boardStates.get(board.mac);
              const info = state?.info;
              const canUpdate = firmwareData && info?.sd_present && !state?.updating;

              return (
                <div
                  key={board.mac}
                  className={`board-card ${selectedBoard === board.mac ? 'selected' : ''} ${state?.updating ? 'updating' : ''}`}
                  onClick={() => setSelectedBoard(board.mac)}
                >
                  <div className="board-header">
                    <span className="mac">{board.mac}</span>
                    <span className={`status ${board.state}`}>{board.state}</span>
                  </div>

                  {state?.loading ? (
                    <div className="loading">Loading firmware info...</div>
                  ) : state?.error ? (
                    <div className="error">{state.error}</div>
                  ) : info ? (
                    <div className="fw-info">
                      <div className="info-row">
                        <span className="label">Version:</span>
                        <span className="value">v{info.version}</span>
                      </div>
                      <div className="info-row">
                        <span className="label">Build:</span>
                        <span className="value">{info.build_date}</span>
                      </div>
                      <div className="info-row">
                        <span className="label">Serial:</span>
                        <span className="value">{info.board_serial}</span>
                      </div>
                      <div className="info-row">
                        <span className="label">SD Card:</span>
                        <span className={`value ${info.sd_present ? 'ok' : 'missing'}`}>
                          {info.sd_present ? 'Present' : 'Missing'}
                        </span>
                      </div>
                    </div>
                  ) : (
                    <div className="no-info">Click to fetch info</div>
                  )}

                  {state?.updating && (
                    <div className="progress-section">
                      <div className="progress-bar">
                        <div className="progress-fill" style={{ width: `${state.progress}%` }} />
                      </div>
                      <span className="progress-text">{state.progressMessage}</span>
                    </div>
                  )}

                  <div className="board-actions">
                    <button
                      className="fetch-btn"
                      onClick={(e) => { e.stopPropagation(); fetchFirmwareInfo(board.mac); }}
                      disabled={state?.loading}
                    >
                      Refresh
                    </button>
                    <button
                      className="update-btn"
                      onClick={(e) => { e.stopPropagation(); updateBoard(board.mac); }}
                      disabled={!canUpdate}
                    >
                      {state?.updating ? 'Updating...' : 'Update'}
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Log */}
      <div className="firmware-section log-section">
        <h3>Log</h3>
        <div className="update-log">
          {log.length === 0 ? (
            <div className="log-line dim">No activity yet.</div>
          ) : (
            log.map((line, i) => (
              <div key={i} className="log-line">{line}</div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
