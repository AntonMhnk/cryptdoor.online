import yaml from 'js-yaml'

export interface VlessKey {
  uuid: string
  server: string
  port: number
  network: string
  security: string
  servername?: string
  publicKey?: string
  shortId?: string
  fingerprint?: string
  flow?: string
  remark: string
  raw: string
}

export class VlessParseError extends Error {}

export function parseVless(link: string): VlessKey {
  const trimmed = link.trim()
  if (!trimmed.startsWith('vless://')) {
    throw new VlessParseError('Link must start with vless://')
  }
  let url: URL
  try {
    url = new URL(trimmed)
  } catch {
    throw new VlessParseError('Invalid link format')
  }

  const uuid = decodeURIComponent(url.username)
  if (!uuid) throw new VlessParseError('Missing UUID')

  const server = url.hostname
  const port = parseInt(url.port, 10)
  if (!server || !port) throw new VlessParseError('Missing host or port')

  const params = url.searchParams
  const network = params.get('type') || 'tcp'
  const security = params.get('security') || 'none'
  const servername = params.get('sni') || params.get('host') || undefined
  const publicKey = params.get('pbk') || undefined
  const shortId = params.get('sid') || undefined
  const fingerprint = params.get('fp') || undefined
  const flow = params.get('flow') || undefined
  const remark = decodeURIComponent(url.hash.replace(/^#/, '')) || `${server}:${port}`

  return {
    uuid,
    server,
    port,
    network,
    security,
    servername,
    publicKey,
    shortId,
    fingerprint,
    flow,
    remark,
    raw: trimmed,
  }
}

export function buildMihomoConfig(key: VlessKey, mixedPort = 7899): string {
  const proxy: Record<string, unknown> = {
    name: 'cryptdoor',
    type: 'vless',
    server: key.server,
    port: key.port,
    uuid: key.uuid,
    network: key.network,
    udp: true,
    tls: key.security === 'reality' || key.security === 'tls',
  }

  if (key.servername) proxy['servername'] = key.servername
  if (key.flow) proxy['flow'] = key.flow

  if (key.security === 'reality' && key.publicKey) {
    proxy['client-fingerprint'] = key.fingerprint || 'chrome'
    proxy['reality-opts'] = {
      'public-key': key.publicKey,
      ...(key.shortId ? { 'short-id': key.shortId } : {}),
    }
  } else if (key.security === 'tls') {
    proxy['client-fingerprint'] = key.fingerprint || 'chrome'
  }

  if (key.network === 'ws') {
    proxy['ws-opts'] = {
      path: '/',
      headers: key.servername ? { Host: key.servername } : {},
    }
  } else if (key.network === 'grpc') {
    proxy['grpc-opts'] = { 'grpc-service-name': '' }
  }

  const config = {
    'mixed-port': mixedPort,
    'allow-lan': false,
    mode: 'rule',
    'log-level': 'info',
    ipv6: false,
    dns: {
      enable: true,
      'enhanced-mode': 'fake-ip',
      'fake-ip-range': '198.18.0.1/16',
      nameserver: ['1.1.1.1', '8.8.8.8'],
      fallback: ['1.0.0.1', '8.8.4.4'],
    },
    proxies: [proxy],
    'proxy-groups': [
      {
        name: 'PROXY',
        type: 'select',
        proxies: ['cryptdoor', 'DIRECT'],
      },
    ],
    rules: ['MATCH,PROXY'],
  }

  return yaml.dump(config, { lineWidth: 120, noRefs: true })
}
