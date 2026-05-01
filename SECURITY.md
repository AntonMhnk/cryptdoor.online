# Security

CryptDoor is a VPN client. We take security and privacy seriously, and we
publish our source code so that anyone can audit what the application does on
their machine.

## Reporting a vulnerability

If you find a security issue, please **do not** open a public GitHub issue —
that would expose users until a fix is shipped.

Instead, report it privately:

- Use GitHub's [private vulnerability reporting](https://github.com/AntonMhnk/cryptdoor.online/security/advisories/new) feature, **or**
- Open a regular issue **without details** asking for a private contact channel.

We will acknowledge the report within a few days and aim to ship a fix in the
next release on the `main` branch.

## What we sign

Every official release published on the [Releases page](https://github.com/AntonMhnk/cryptdoor.online/releases)
is signed with [minisign](https://jedisct1.github.io/minisign/). The public
key used to verify the signatures is embedded in the application's binary
(see `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`).

This means:

- Auto-updates verify the signature of every downloaded installer before
  applying it. A tampered binary will be rejected.
- You can manually verify a downloaded `.exe` or `.app.tar.gz` against its
  `.sig` file using `minisign -V`.

The corresponding **private signing key never leaves the maintainer's
machine** and is uploaded to GitHub Actions as a secret only for the
purpose of signing CI-built releases.

## Reproducible builds

The release pipeline lives entirely in `.github/workflows/release.yml`. Every
binary attached to a release is built by GitHub Actions from a tagged commit
on the public `main` branch. You can:

1. Check out the same tag locally.
2. Run `pnpm tauri build --target <triple>` with the matching Rust toolchain.
3. Compare the resulting binary against the released one.

Differences should be limited to embedded timestamps and signatures.

## What CryptDoor does NOT do

- It does **not** collect telemetry of any kind.
- It does **not** phone home to any server other than the VPN you connect to
  and the GitHub release endpoint (only when checking for updates).
- It does **not** store your VLESS keys anywhere outside your local
  `localStorage` (browser-style storage, on-disk, never synced).
- It does **not** log your traffic.

These claims are verifiable from the source code — that's the whole point of
making it public.
