#!/usr/bin/env node
// Copies the freshly-built `cryptdoor-helper` binary into src-tauri/sidecar/
// using the rust target triple in the filename — this is the layout Tauri's
// `externalBin` mechanism expects.
//
// Result file:
//   - sidecar/cryptdoor-helper-<triple>          (macOS / Linux)
//   - sidecar/cryptdoor-helper-<triple>.exe      (Windows)

import { execSync } from 'node:child_process'
import { copyFileSync, chmodSync, mkdirSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')
const tauriDir = join(root, 'src-tauri')
const sidecar = join(tauriDir, 'sidecar')

const isWin = process.platform === 'win32'
const exeExt = isWin ? '.exe' : ''

const triple = (() => {
  if (process.env.CARGO_BUILD_TARGET) return process.env.CARGO_BUILD_TARGET
  try {
    const out = execSync('rustc -vV').toString()
    const m = out.match(/(?<=host: ).+(?=\s*)/g)
    if (m && m[0]) return m[0].trim()
  } catch (e) {
    console.error('rustc not found:', e.message)
    process.exit(1)
  }
  return null
})()

if (!triple) {
  console.error('could not detect rustc host triple')
  process.exit(1)
}

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
const helperBinName = `cryptdoor-helper${exeExt}`

// `cargo build --target <triple>` puts artifacts under `target/<triple>/<profile>/`,
// while plain `cargo build` puts them under `target/<profile>/`. Try the
// triple-aware path first if CARGO_BUILD_TARGET is set, then fall back.
const cargoTarget = process.env.CARGO_BUILD_TARGET || ''
const candidates = [
  cargoTarget && join(targetDir, cargoTarget, profile, helperBinName),
  join(targetDir, profile, helperBinName),
  join(targetDir, profile === 'release' ? 'debug' : 'release', helperBinName),
  cargoTarget &&
    join(
      targetDir,
      cargoTarget,
      profile === 'release' ? 'debug' : 'release',
      helperBinName,
    ),
].filter(Boolean)

let src = candidates.find(p => existsSync(p))
if (!src) {
  console.error('helper not found. tried:')
  for (const c of candidates) console.error('  ' + c)
  console.error('run: cd src-tauri && cargo build --bin cryptdoor-helper')
  process.exit(1)
}

mkdirSync(sidecar, { recursive: true })

// File name expected by Tauri's externalBin: "<base>-<triple><ext>"
const dest = join(sidecar, `cryptdoor-helper-${triple}${exeExt}`)
copyFileSync(src, dest)
if (!isWin) {
  chmodSync(dest, 0o755)
}
console.log(`helper -> ${dest}`)
