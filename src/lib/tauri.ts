import { invoke } from '@tauri-apps/api/core'

export interface ConnectionStatus {
  running: boolean
  port: number
  mode: string
  helperInstalled: boolean
}

export interface HelperStatus {
  installed: boolean
  socketReady: boolean
  version: string | null
}

interface RawConnectionStatus {
  running: boolean
  port: number
  mode: string
  helper_installed: boolean
}

interface RawHelperStatus {
  installed: boolean
  socketReady: boolean
  version: string | null
}

function normalizeStatus(s: RawConnectionStatus): ConnectionStatus {
  return {
    running: s.running,
    port: s.port,
    mode: s.mode,
    helperInstalled: s.helper_installed,
  }
}

export const api = {
  async connect(configYaml: string): Promise<ConnectionStatus> {
    return normalizeStatus(
      await invoke<RawConnectionStatus>('connect_proxy', {
        args: { config_yaml: configYaml },
      }),
    )
  },
  async disconnect(): Promise<ConnectionStatus> {
    return normalizeStatus(await invoke<RawConnectionStatus>('disconnect_proxy'))
  },
  async status(): Promise<ConnectionStatus> {
    return normalizeStatus(await invoke<RawConnectionStatus>('connection_status'))
  },
  externalIp(): Promise<string> {
    return invoke<string>('current_external_ip')
  },
  helperStatus(): Promise<HelperStatus> {
    return invoke<RawHelperStatus>('helper_status') as Promise<HelperStatus>
  },
  installHelper(): Promise<void> {
    return invoke<void>('install_helper')
  },
  setTrayStatus(label: string, connected: boolean): Promise<void> {
    return invoke<void>('tray_set_status', { label, connected })
  },
  showWindow(): Promise<void> {
    return invoke<void>('window_show')
  },
  installUpdate(): Promise<void> {
    return invoke<void>('install_update')
  },
  restartApp(): Promise<void> {
    return invoke<void>('restart_app')
  },
}
