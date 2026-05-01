// Downloads wintun.dll (TUN driver from Wireguard) and puts it next to the
// mihomo sidecar so it gets bundled by Tauri on Windows builds.
//
// Usage: node scripts/download-wintun.mjs [target]
//   target: rust target triple (default: detected from rustc)
//
// Wintun.dll source: https://www.wintun.net/

import { execSync } from 'node:child_process'
import { createWriteStream, existsSync } from 'node:fs'
import fs from 'node:fs/promises'
import path from 'node:path'
import { pipeline } from 'node:stream/promises'
import fetch from 'node-fetch'

const WINTUN_VERSION = '0.14.1'
const WINTUN_URL = `https://www.wintun.net/builds/wintun-${WINTUN_VERSION}.zip`

const cwd = process.cwd()
const SIDECAR_DIR = path.join(cwd, 'src-tauri', 'sidecar')
const FORCE = process.argv.includes('--force') || process.argv.includes('-f')

const ARCH_MAP = {
  'x86_64-pc-windows-msvc': 'amd64',
  'aarch64-pc-windows-msvc': 'arm64',
  'i686-pc-windows-msvc': 'x86',
}

const target =
  process.argv.slice(2).find(a => !a.startsWith('-')) ||
  execSync('rustc -vV').toString().match(/(?<=host: ).+(?=\s*)/g)[0]

if (!target.includes('windows')) {
  console.log(`skipping wintun for non-windows target: ${target}`)
  process.exit(0)
}

const arch = ARCH_MAP[target]
if (!arch) {
  console.error(`unsupported windows target: ${target}`)
  process.exit(1)
}

const dest = path.join(SIDECAR_DIR, 'wintun.dll')

async function main() {
  await fs.mkdir(SIDECAR_DIR, { recursive: true })

  if (!FORCE && existsSync(dest)) {
    console.log(`wintun.dll already exists: ${dest}`)
    return
  }

  console.log(`Downloading ${WINTUN_URL}`)
  const res = await fetch(WINTUN_URL)
  if (!res.ok) throw new Error(`download failed: ${res.status}`)

  const tmpDir = path.join(SIDECAR_DIR, '.wintun-tmp')
  await fs.mkdir(tmpDir, { recursive: true })
  const archive = path.join(tmpDir, `wintun-${WINTUN_VERSION}.zip`)
  await pipeline(res.body, createWriteStream(archive))

  const AdmZip = (await import('adm-zip')).default
  const zip = new AdmZip(archive)

  const expectedEntry = `wintun/bin/${arch}/wintun.dll`
  const entry = zip.getEntries().find(e => e.entryName === expectedEntry)
  if (!entry) {
    throw new Error(`wintun.dll for ${arch} not found in zip (looked for ${expectedEntry})`)
  }

  await fs.writeFile(dest, entry.getData())
  await fs.rm(tmpDir, { recursive: true, force: true })
  console.log(`wintun.dll ready: ${dest}`)
}

main().catch(err => {
  console.error(err)
  process.exit(1)
})
