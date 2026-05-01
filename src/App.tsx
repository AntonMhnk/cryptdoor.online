import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart'
import { api, HelperStatus } from './lib/tauri'
import {
  loadActiveId,
  loadKeys,
  makeId,
  saveActiveId,
  saveKeys,
  StoredKey,
} from './lib/storage'
import { buildMihomoConfig, parseVless, VlessParseError } from './lib/vless'

type Status = 'idle' | 'preparing' | 'installing' | 'connecting' | 'connected' | 'disconnecting'

const isWindows =
  typeof navigator !== 'undefined' &&
  /windows/i.test(navigator.userAgent || navigator.platform || '')

const PowerIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
    <line x1="12" y1="2" x2="12" y2="12" />
  </svg>
)

export default function App() {
  const [keys, setKeys] = useState<StoredKey[]>(() => loadKeys())
  const [activeId, setActiveId] = useState<string | null>(() => loadActiveId())
  const [status, setStatus] = useState<Status>('idle')
  const [error, setError] = useState<string | null>(null)
  const [ip, setIp] = useState<string>('')
  const [draft, setDraft] = useState('')
  const [helper, setHelper] = useState<HelperStatus>({
    installed: false,
    socketReady: false,
    version: null,
  })
  const [autostart, setAutostart] = useState<boolean>(false)
  const [version, setVersion] = useState<string>('')
  const [updateReady, setUpdateReady] = useState<boolean>(false)

  const activeKey = useMemo(
    () => keys.find(k => k.id === activeId) ?? null,
    [keys, activeId],
  )

  useEffect(() => { saveKeys(keys) }, [keys])
  useEffect(() => { saveActiveId(activeId) }, [activeId])

  const refreshHelper = useCallback(async () => {
    try {
      const h = await api.helperStatus()
      setHelper(h)
    } catch {}
  }, [])

  useEffect(() => {
    refreshHelper()
    api.status().then(s => {
      if (s.running) setStatus('connected')
    }).catch(() => {})
    isAutostartEnabled().then(setAutostart).catch(() => {})
    getVersion().then(setVersion).catch(() => {})
  }, [refreshHelper])

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    listen('update-installed', () => setUpdateReady(true)).then(fn => { unlisten = fn })
    return () => { unlisten?.() }
  }, [])

  const toggleAutostart = useCallback(async () => {
    try {
      if (autostart) {
        await disableAutostart()
        setAutostart(false)
      } else {
        await enableAutostart()
        setAutostart(true)
      }
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Could not change autostart')
    }
  }, [autostart])

  const handleConnectRef = useRef<() => void>(() => {})
  const handleDisconnectRef = useRef<() => void>(() => {})

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    listen<string>('tray-action', evt => {
      if (evt.payload === 'connect') handleConnectRef.current()
      else if (evt.payload === 'disconnect') handleDisconnectRef.current()
    }).then(fn => { unlisten = fn })
    return () => { unlisten?.() }
  }, [])

  const refreshIp = useCallback(async () => {
    try {
      setIp(await api.externalIp())
    } catch {
      setIp('')
    }
  }, [])

  useEffect(() => {
    if (status === 'connected' || status === 'idle') refreshIp()
  }, [status, refreshIp])

  const addKey = useCallback(() => {
    setError(null)
    const text = draft.trim()
    if (!text) return
    try {
      const parsed = parseVless(text)
      const stored: StoredKey = { id: makeId(), key: parsed, addedAt: Date.now() }
      setKeys(prev => [...prev, stored])
      setActiveId(stored.id)
      setDraft('')
    } catch (e) {
      setError(e instanceof VlessParseError ? e.message : String(e))
    }
  }, [draft])

  const handleDisconnect = useCallback(async () => {
    setStatus('disconnecting')
    try { await api.disconnect() } catch {}
    setStatus('idle')
    setTimeout(refreshIp, 500)
  }, [refreshIp])

  const deleteKey = useCallback(
    (id: string) => {
      setKeys(prev => prev.filter(k => k.id !== id))
      if (activeId === id) {
        setActiveId(null)
        if (status === 'connected') handleDisconnect().catch(() => {})
      }
    },
    [activeId, status, handleDisconnect],
  )

  const handleConnect = useCallback(async () => {
    if (!activeKey) {
      setError('Add and select a key first')
      return
    }
    setError(null)
    try {
      const yaml = buildMihomoConfig(activeKey.key)
      if (!helper.installed) {
        setStatus('installing')
      } else {
        setStatus('preparing')
      }
      const connectPromise = api.connect(yaml)
      if (!helper.installed) {
        await refreshHelper()
      }
      setStatus('connecting')
      await connectPromise
      setStatus('connected')
      await refreshHelper()
      setTimeout(refreshIp, 1500)
    } catch (e) {
      setStatus('idle')
      setError(typeof e === 'string' ? e : 'Connection failed')
      refreshHelper()
    }
  }, [activeKey, helper.installed, refreshHelper, refreshIp])

  const onPowerClick = useCallback(() => {
    if (status === 'connected') handleDisconnect()
    else if (status === 'idle') handleConnect()
  }, [status, handleConnect, handleDisconnect])

  const busy = status !== 'idle' && status !== 'connected'
  const overlayText =
    status === 'preparing' ? 'Preparing config…' :
    status === 'installing' ? 'First launch — enter your macOS password' :
    status === 'connecting' ? 'Bringing up TUN…' :
    status === 'disconnecting' ? 'Disconnecting…' : ''

  const statusLabel =
    status === 'connected' ? 'Connected' :
    status === 'connecting' ? 'Connecting…' :
    status === 'installing' ? 'Authorizing…' :
    status === 'preparing' ? 'Preparing…' :
    status === 'disconnecting' ? 'Disconnecting…' :
    'Disconnected'

  useEffect(() => {
    handleConnectRef.current = handleConnect
    handleDisconnectRef.current = handleDisconnect
  }, [handleConnect, handleDisconnect])

  useEffect(() => {
    api.setTrayStatus(statusLabel, status === 'connected').catch(() => {})
  }, [statusLabel, status])

  return (
    <div className="app">
      <div className="titlebar" />
      {version && (
        <div className="version-tag" title={updateReady ? 'Update will be applied on next restart' : `CryptDoor v${version}`}>
          v{version}
          {updateReady && <span className="version-dot" aria-label="Update pending" />}
        </div>
      )}
      <div className="shell">
        <div className="brand">
          <span className="brand-dot" />
          CryptDoor
        </div>

        <div className="power">
          <button
            className={`power-btn ${status === 'connected' ? 'on' : ''}`}
            onClick={onPowerClick}
            disabled={busy || (!activeKey && status === 'idle')}
            aria-label={status === 'connected' ? 'Disconnect' : 'Connect'}
          >
            <PowerIcon />
          </button>
          <div className="power-status">
            <div className="label">{statusLabel}</div>
            {ip && <div className="ip">IP: {ip}</div>}
          </div>
        </div>

        {error && <div className="error">{error}</div>}

        <label className="row-toggle">
          <input
            type="checkbox"
            checked={autostart}
            onChange={toggleAutostart}
          />
          <span>Launch CryptDoor at login</span>
        </label>

        {!helper.installed && status === 'idle' && (
          <div className="hint">
            {isWindows
              ? 'On first connect Windows will show a UAC prompt — once. CryptDoor needs admin rights to route all traffic (including Telegram) through the VPN.'
              : 'On first connect macOS will ask for your password — once. CryptDoor needs this to route all traffic (including Telegram) through the VPN.'}
          </div>
        )}

        <div>
          <div className="section-title">Keys</div>
          {keys.length === 0 ? (
            <div className="empty">No keys yet. Paste a vless:// link below.</div>
          ) : (
            <div className="keys">
              {keys.map(k => (
                <div
                  key={k.id}
                  className={`key ${activeId === k.id ? 'active' : ''}`}
                  onClick={() => setActiveId(k.id)}
                >
                  <span className="key-radio" />
                  <div className="key-info">
                    <div className="key-name">{k.key.remark}</div>
                    <div className="key-host">{k.key.server}:{k.key.port}</div>
                  </div>
                  <button
                    className="key-delete"
                    onClick={e => { e.stopPropagation(); deleteKey(k.id) }}
                    aria-label="Delete"
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="add-key">
          <div className="section-title">Add key</div>
          <textarea
            placeholder="vless://uuid@host:port?...#name"
            value={draft}
            onChange={e => setDraft(e.target.value)}
            spellCheck={false}
          />
          <button onClick={addKey} disabled={!draft.trim()}>
            Add
          </button>
        </div>
      </div>

      {busy && (
        <div className="overlay">
          <div className="spinner" />
          <div className="overlay-text">{overlayText}</div>
        </div>
      )}
    </div>
  )
}
