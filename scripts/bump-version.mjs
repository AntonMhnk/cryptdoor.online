#!/usr/bin/env node
// Bump version across package.json, src-tauri/tauri.conf.json and src-tauri/Cargo.toml.
// Usage:
//   node scripts/bump-version.mjs <version>     -> set explicit version (e.g. 0.2.0)
//   node scripts/bump-version.mjs patch         -> bump patch
//   node scripts/bump-version.mjs minor         -> bump minor
//   node scripts/bump-version.mjs major         -> bump major

import fs from 'node:fs/promises'
import path from 'node:path'
import { execSync } from 'node:child_process'

const root = process.cwd()

function bumpSemver(current, kind) {
  const m = current.match(/^(\d+)\.(\d+)\.(\d+)$/)
  if (!m) throw new Error(`invalid semver: ${current}`)
  let [_, maj, min, pat] = m
  maj = +maj
  min = +min
  pat = +pat
  if (kind === 'major') return `${maj + 1}.0.0`
  if (kind === 'minor') return `${maj}.${min + 1}.0`
  if (kind === 'patch') return `${maj}.${min}.${pat + 1}`
  throw new Error(`unknown bump: ${kind}`)
}

async function readJson(p) {
  return JSON.parse(await fs.readFile(p, 'utf8'))
}

async function writeJson(p, obj) {
  const out = JSON.stringify(obj, null, 2) + '\n'
  await fs.writeFile(p, out)
}

async function main() {
  const arg = process.argv[2]
  if (!arg) {
    console.error('usage: bump-version.mjs <version|patch|minor|major>')
    process.exit(1)
  }

  const pkgPath = path.join(root, 'package.json')
  const tauriPath = path.join(root, 'src-tauri', 'tauri.conf.json')
  const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml')

  const pkg = await readJson(pkgPath)
  const tauriCfg = await readJson(tauriPath)

  const next = ['major', 'minor', 'patch'].includes(arg)
    ? bumpSemver(pkg.version, arg)
    : arg

  if (!/^\d+\.\d+\.\d+$/.test(next)) {
    throw new Error(`bad version: ${next}`)
  }

  pkg.version = next
  await writeJson(pkgPath, pkg)

  tauriCfg.version = next
  await writeJson(tauriPath, tauriCfg)

  let cargo = await fs.readFile(cargoPath, 'utf8')
  cargo = cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${next}"`)
  await fs.writeFile(cargoPath, cargo)

  console.log(`bumped to ${next}`)
  console.log()
  console.log('next steps:')
  console.log(`  git commit -am "release v${next}"`)
  console.log(`  git tag v${next}`)
  console.log(`  git push && git push --tags`)
}

main().catch(err => {
  console.error(err)
  process.exit(1)
})
