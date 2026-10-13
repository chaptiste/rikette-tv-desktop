#!/usr/bin/env node
/* eslint-disable @typescript-eslint/explicit-function-return-type -- Node script. */
/**
 * Builds obs-runtime/: the parts of OBS Studio Rikette records with, in OBS's own layout,
 * plus FFmpeg and rikette-capture next to obs.dll. Tauri bundles the folder as `obs/`.
 *
 * Downloads (once, checksummed, cached in .cache/obs) the OBS release that the `libobs`
 * crate is pinned to, and the obs-deps build OBS was compiled against, which has the
 * ffmpeg.exe and ffprobe.exe that share OBS's FFmpeg DLLs. Windows only.
 *
 * Usage: pnpm --filter desktop obs-runtime [--debug] [--no-capture]
 */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  cpSync,
  createReadStream,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync
} from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const OBS = {
  version: '32.0.4',
  url: 'https://github.com/obsproject/obs-studio/releases/download/32.0.4/OBS-Studio-32.0.4-Windows-x64.zip',
  sha256: 'fd367a7f156319426f1a600720c1288ad0011da168dff41e0f94dbb18cc1dc0f'
}
// buildspec.json of OBS 32.0.4.
const DEPS = {
  version: '2025-08-23',
  url: 'https://github.com/obsproject/obs-deps/releases/download/2025-08-23/windows-deps-2025-08-23-x64.zip',
  sha256: '8de229cff6f1981508c0eb646b35e644633a5855787b9f5d3b90ae2aeb87ffc1'
}

/** The plugins Rikette loads, and everything they and libobs link against. */
const PLUGINS = ['win-capture', 'win-wasapi', 'obs-ffmpeg', 'obs-nvenc', 'obs-qsv11', 'obs-x264']
const BIN = [
  'obs.dll',
  'libobs-d3d11.dll',
  'libobs-winrt.dll',
  'w32-pthreads.dll',
  'zlib.dll',
  'avcodec-61.dll',
  'avdevice-61.dll',
  'avfilter-10.dll',
  'avformat-61.dll',
  'avutil-59.dll',
  'swresample-5.dll',
  'swscale-8.dll',
  'libx264-164.dll',
  'libcurl.dll',
  'librist.dll',
  'srt.dll',
  // Run by the plugins: the replay buffer's muxer and the encoder probes.
  'obs-ffmpeg-mux.exe',
  'obs-nvenc-test.exe',
  'obs-qsv-test.exe',
  'obs-amf-test.exe'
]
/** Shipped app-local so users do not need the Visual C++ redistributable. */
const VC_RUNTIME = ['msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll']

const desktop = join(dirname(fileURLToPath(import.meta.url)), '..')
const out = join(desktop, 'obs-runtime')
const cache = join(desktop, '.cache', 'obs')
const args = new Set(process.argv.slice(2))

if (process.platform !== 'win32') {
  console.log('obs-runtime: libobs is only bundled on Windows; nothing to do here.')
  process.exit(0)
}

const obsDir = await fetchZip('obs', OBS)
const depsDir = await fetchZip('deps', DEPS)

for (const entry of readdirSync(out)) {
  if (entry !== 'README.md') rmSync(join(out, entry), { recursive: true, force: true })
}
const bin = join(out, 'bin', '64bit')
mkdirSync(bin, { recursive: true })
for (const file of BIN) copyFileSync(join(obsDir, 'bin', '64bit', file), join(bin, file))
for (const file of ['ffmpeg.exe', 'ffprobe.exe'])
  copyFileSync(join(depsDir, 'bin', file), join(bin, file))
for (const file of VC_RUNTIME) {
  copyFileSync(join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', file), join(bin, file))
}

const plugins = join(out, 'obs-plugins', '64bit')
mkdirSync(plugins, { recursive: true })
for (const plugin of PLUGINS) {
  copyFileSync(
    join(obsDir, 'obs-plugins', '64bit', `${plugin}.dll`),
    join(plugins, `${plugin}.dll`)
  )
  copyData(join(obsDir, 'data', 'obs-plugins', plugin), join(out, 'data', 'obs-plugins', plugin))
}
copyData(join(obsDir, 'data', 'libobs'), join(out, 'data', 'libobs'))

if (!args.has('--no-capture')) {
  const profile = args.has('--debug') ? 'debug' : 'release'
  run('cargo', ['build', '-p', 'rikette-capture', ...(profile === 'release' ? ['--release'] : [])])
  copyFileSync(
    join(desktop, 'target', profile, 'rikette-capture.exe'),
    join(bin, 'rikette-capture.exe')
  )
}

console.log(
  `obs-runtime: OBS ${OBS.version} (${megabytes(out)} MB) in ${relative(process.cwd(), out)}`
)

/** Plugin data without debug symbols or translations other than English. */
function copyData(from, to) {
  cpSync(from, to, {
    recursive: true,
    filter: (path) => {
      if (path.endsWith('.pdb')) return false
      if (/[\\/]locale[\\/]/.test(path) && !path.endsWith('en-US.ini'))
        return statSync(path).isDirectory()
      return true
    }
  })
}

async function fetchZip(name, { url, sha256 }) {
  const dir = join(cache, `${name}-${sha256.slice(0, 12)}`)
  if (existsSync(join(dir, '.done'))) return dir
  mkdirSync(cache, { recursive: true })
  const zip = join(cache, `${name}.zip`)
  if (!existsSync(zip) || (await hashFile(zip)) !== sha256) {
    console.log(`obs-runtime: downloading ${url}`)
    const res = await fetch(url)
    if (!res.ok) throw new Error(`GET ${url} ${res.status}`)
    await writeFile(zip, Buffer.from(await res.arrayBuffer()))
    const actual = await hashFile(zip)
    if (actual !== sha256) throw new Error(`${url}: sha256 ${actual}, want ${sha256}`)
  }
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
  // Windows 10+ ships bsdtar, which reads zip.
  run('tar', ['-xf', zip, '-C', dir])
  await writeFile(join(dir, '.done'), '')
  return dir
}

function hashFile(file) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256')
    createReadStream(file)
      .on('data', (chunk) => hash.update(chunk))
      .on('end', () => resolve(hash.digest('hex')))
      .on('error', reject)
  })
}

function run(command, commandArgs) {
  execFileSync(command, commandArgs, { cwd: desktop, stdio: 'inherit' })
}

function megabytes(dir) {
  let total = 0
  const walk = (path) => {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const full = join(path, entry.name)
      if (entry.isDirectory()) walk(full)
      else total += statSync(full).size
    }
  }
  walk(dir)
  return Math.round(total / 1024 / 1024)
}
