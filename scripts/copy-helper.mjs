#!/usr/bin/env node
import { execSync } from 'node:child_process'
import { copyFileSync, chmodSync, mkdirSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')
const tauriDir = join(root, 'src-tauri')
const sidecar = join(tauriDir, 'sidecar')

let targetDir
try {
  const meta = JSON.parse(
    execSync('cargo metadata --no-deps --format-version 1', {
      cwd: tauriDir,
      stdio: ['ignore', 'pipe', 'inherit'],
    }).toString(),
  )
  targetDir = meta.target_directory
} catch (e) {
  console.error('cargo metadata failed:', e.message)
  process.exit(1)
}

const profile = process.env.PROFILE === 'release' ? 'release' : 'debug'
let src = join(targetDir, profile, 'cryptdoor-helper')
if (!existsSync(src)) {
  const fallback = join(targetDir, profile === 'release' ? 'debug' : 'release', 'cryptdoor-helper')
  if (existsSync(fallback)) {
    src = fallback
  } else {
    console.error(`helper not found at ${src}`)
    console.error('run: cd src-tauri && cargo build --bin cryptdoor-helper')
    process.exit(1)
  }
}

mkdirSync(sidecar, { recursive: true })
const dest = join(sidecar, 'cryptdoor-helper')
copyFileSync(src, dest)
chmodSync(dest, 0o755)
console.log(`helper -> ${dest}`)
