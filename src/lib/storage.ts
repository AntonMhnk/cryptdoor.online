import { VlessKey } from './vless'

export interface StoredKey {
  id: string
  key: VlessKey
  addedAt: number
}

const KEYS = 'cryptdoor.keys.v1'
const ACTIVE = 'cryptdoor.active.v1'

export function loadKeys(): StoredKey[] {
  try {
    const raw = localStorage.getItem(KEYS)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

export function saveKeys(keys: StoredKey[]) {
  localStorage.setItem(KEYS, JSON.stringify(keys))
}

export function loadActiveId(): string | null {
  return localStorage.getItem(ACTIVE)
}

export function saveActiveId(id: string | null) {
  if (id) localStorage.setItem(ACTIVE, id)
  else localStorage.removeItem(ACTIVE)
}

export function makeId(): string {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36)
}
