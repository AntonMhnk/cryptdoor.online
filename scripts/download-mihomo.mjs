import { execSync } from 'node:child_process'
import { createWriteStream, existsSync } from 'node:fs'
import fs from 'node:fs/promises'
import path from 'node:path'
import { pipeline } from 'node:stream/promises'
import zlib from 'node:zlib'
import fetch from 'node-fetch'

const cwd = process.cwd()
const SIDECAR_DIR = path.join(cwd, 'src-tauri', 'sidecar')
const FORCE = process.argv.includes('--force') || process.argv.includes('-f')

const PLATFORM_MAP = {
  'aarch64-apple-darwin': { os: 'darwin', arch: 'arm64-go122' },
  'x86_64-apple-darwin': { os: 'darwin', arch: 'amd64-v2-go122' },
  'aarch64-pc-windows-msvc': { os: 'windows', arch: 'arm64' },
  'x86_64-pc-windows-msvc': { os: 'windows', arch: 'amd64-v2' },
  'x86_64-unknown-linux-gnu': { os: 'linux', arch: 'amd64-v2' },
  'aarch64-unknown-linux-gnu': { os: 'linux', arch: 'arm64' },
}

const target =
  process.argv.slice(2).find(a => !a.startsWith('-')) ||
  execSync('rustc -vV').toString().match(/(?<=host: ).+(?=\s*)/g)[0]

const meta = PLATFORM_MAP[target]
if (!meta) {
  console.error(`Unsupported target: ${target}`)
  process.exit(1)
}

const isWin = meta.os === 'windows'
const ext = isWin ? '.zip' : '.gz'
const binName = isWin ? 'mihomo.exe' : 'mihomo'
const sidecarName = `mihomo-${target}${isWin ? '.exe' : ''}`
const sidecarPath = path.join(SIDECAR_DIR, sidecarName)

async function getLatestVersion() {
  const res = await fetch(
    'https://github.com/MetaCubeX/mihomo/releases/latest/download/version.txt',
  )
  if (!res.ok) throw new Error(`version.txt fetch failed: ${res.status}`)
  return (await res.text()).trim()
}

async function main() {
  await fs.mkdir(SIDECAR_DIR, { recursive: true })

  if (!FORCE && existsSync(sidecarPath)) {
    console.log(`mihomo already exists: ${sidecarPath}`)
    return
  }

  const version = await getLatestVersion()
  const baseName = `mihomo-${meta.os}-${meta.arch}`
  const fileName = `${baseName}-${version}${ext}`
  const url = `https://github.com/MetaCubeX/mihomo/releases/download/${version}/${fileName}`

  console.log(`Downloading ${url}`)
  const res = await fetch(url)
  if (!res.ok) throw new Error(`download failed: ${res.status}`)

  const tmpDir = path.join(SIDECAR_DIR, '.tmp')
  await fs.mkdir(tmpDir, { recursive: true })
  const archive = path.join(tmpDir, fileName)
  await pipeline(res.body, createWriteStream(archive))

  if (isWin) {
    const AdmZip = (await import('adm-zip')).default
    const zip = new AdmZip(archive)
    zip.extractAllTo(tmpDir, true)
    const extracted = (await fs.readdir(tmpDir)).find(
      f => f.toLowerCase() === binName || f.endsWith('.exe'),
    )
    if (!extracted) throw new Error('mihomo.exe not found in zip')
    await fs.rename(path.join(tmpDir, extracted), sidecarPath)
  } else {
    const buf = await fs.readFile(archive)
    const decompressed = zlib.gunzipSync(buf)
    await fs.writeFile(sidecarPath, decompressed)
    await fs.chmod(sidecarPath, 0o755)
  }

  await fs.rm(tmpDir, { recursive: true, force: true })
  console.log(`Mihomo ready: ${sidecarPath}`)
}

main().catch(err => {
  console.error(err)
  process.exit(1)
})
