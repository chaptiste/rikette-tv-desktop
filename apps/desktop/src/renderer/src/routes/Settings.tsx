import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  BUFFER_SECONDS,
  NVENC_PRESETS,
  OUTPUT_RES,
  defaultAppSettings,
  formatHotkey,
  qualityPresets,
  type AppSettings,
  type CaptureMode,
  type EncoderPref,
  type NvencPreset,
  type OutputRes,
  type QualityPreset
} from '@echoframe/shared'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Input } from '@/components/ui/input'
import { Kbd } from '@/components/ui/kbd'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { PageHeader } from '@/components/PageHeader'
import { eventToAccelerator } from '@/lib/accelerator'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import type { DisplayInfo, CaptureStatus, AudioDeviceInfo, WindowInfo } from '../../../tauri/api'

const captureModes: { value: CaptureMode; label: string }[] = [
  { value: 'auto-game', label: 'Auto game' },
  { value: 'display', label: 'Display' },
  { value: 'window', label: 'Window' }
]

const tabs = [
  { id: 'capture', label: 'Capture' },
  { id: 'audio', label: 'Audio' },
  { id: 'video', label: 'Video' },
  { id: 'controls', label: 'Controls' },
  { id: 'system', label: 'System' }
] as const

type Tab = (typeof tabs)[number]['id']

export function Settings(): React.JSX.Element {
  const [settings, setSettings] = useState<AppSettings | null>(() =>
    hasDesktopApi() ? null : defaultAppSettings('Videos/Rikette')
  )
  const [tab, setTab] = useState<Tab>('capture')
  const [displays, setDisplays] = useState<DisplayInfo[]>([])
  const [audioDevices, setAudioDevices] = useState<{
    outputs: AudioDeviceInfo[]
    inputs: AudioDeviceInfo[]
  }>({ outputs: [], inputs: [] })
  const [hotkeyRegistered, setHotkeyRegistered] = useState(() => hasDesktopApi())
  const [recordingHotkey, setRecordingHotkey] = useState(false)
  const [capture, setCapture] = useState<CaptureStatus | null>(null)
  const [windows, setWindows] = useState<WindowInfo[]>([])

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getSettings().then(setSettings)
    void ipc.listDisplays().then(setDisplays)
    void ipc.listAudioDevices().then(setAudioDevices)
    void ipc.hotkeyRegistered().then(setHotkeyRegistered)
    void ipc.getCaptureStatus().then(setCapture)
    return ipc.onCaptureStatus((status) => {
      setCapture(status)
      if (status.ready) void ipc.listAudioDevices().then(setAudioDevices)
    })
  }, [])

  const windowMode = settings?.captureMode === 'window'
  useEffect(() => {
    if (windowMode && hasDesktopApi()) void ipc.listWindows().then(setWindows)
  }, [windowMode])

  useEffect(() => {
    if (!recordingHotkey) return

    const onKeyDown = (event: KeyboardEvent): void => {
      event.preventDefault()
      event.stopPropagation()
      const accelerator = eventToAccelerator(event, hasDesktopApi() ? ipc.platform : 'win32')
      if (!accelerator) return
      setRecordingHotkey(false)
      void patch({ hotkey: accelerator })
    }

    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [recordingHotkey])

  async function patch(next: Partial<AppSettings>): Promise<void> {
    if (!hasDesktopApi()) {
      setSettings((prev) => (prev ? { ...prev, ...next } : prev))
      return
    }
    try {
      const result = await ipc.updateSettings(next)
      setSettings(result.settings)
      setHotkeyRegistered(result.hotkeyRegistered)
      if (result.capture) setCapture(result.capture)
    } catch (error) {
      toast.error('Could not update settings', {
        description: error instanceof Error ? error.message : String(error)
      })
    }
  }

  function refreshWindows(): void {
    if (hasDesktopApi()) void ipc.listWindows().then(setWindows)
  }

  async function browseOutput(): Promise<void> {
    if (!hasDesktopApi()) return
    setSettings(await ipc.pickOutputFolder())
  }

  if (!settings) {
    return (
      <main className="flex h-full flex-col">
        <PageHeader kicker="Options" title="Settings" />
        <div className="flex flex-1 items-center justify-center">
          <p className="eyebrow animate-pulse">Loading…</p>
        </div>
      </main>
    )
  }

  const hotkeyLabel = formatHotkey(settings.hotkey, hasDesktopApi() ? ipc.platform : 'win32')
  const encoderId = capture?.encoderId || settings.encoderId
  const nvenc =
    settings.encoderPref === 'jim_nvenc' ||
    settings.encoderPref === 'ffmpeg_nvenc' ||
    (settings.encoderPref === 'auto' && encoderId.includes('nvenc'))
  const monitors =
    displays.length > 0 ? displays : [{ index: 0, label: 'Display 1', primary: true }]
  const currentIndex = Math.max(
    0,
    tabs.findIndex((item) => item.id === tab)
  )
  const current = tabs[currentIndex] ?? tabs[0]

  return (
    <main className="flex h-full flex-col">
      <PageHeader kicker="Options" title="Settings" />
      <div className="flex min-h-0 flex-1 gap-6 px-8 pb-8">
        <nav className="flex w-44 shrink-0 flex-col border-t border-border">
          {tabs.map((item, index) => {
            const active = item.id === tab
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setTab(item.id)}
                style={{ '--i': index } as React.CSSProperties}
                className={cn(
                  'slide-in-left group relative flex h-11 items-center gap-3 border-b border-border px-3 font-display text-[17px] font-bold transition-colors duration-150',
                  active
                    ? 'bg-card text-foreground shadow-[inset_3px_0_0_var(--primary)]'
                    : 'text-muted-foreground hover:bg-accent hover:text-foreground'
                )}
              >
                <span
                  className={cn(
                    'tc text-[10px] font-semibold',
                    active ? 'text-primary' : 'text-border-strong'
                  )}
                >
                  {(index + 1).toString().padStart(2, '0')}
                </span>
                {item.label}
              </button>
            )
          })}
        </nav>

        <section className="card flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="flex items-baseline justify-between border-b border-border px-5 py-3.5">
            <h2 className="display-title text-[24px]">{current.label}</h2>
            <span className="eyebrow">
              {(currentIndex + 1).toString().padStart(2, '0')} /{' '}
              {tabs.length.toString().padStart(2, '0')}
            </span>
          </div>
          <div key={tab} className="min-h-0 flex-1 overflow-y-auto">
            {tab === 'capture' ? (
              <Rows>
                <Row label="Mode" hint="Auto game hooks the game you are playing">
                  <FieldSelect
                    value={settings.captureMode}
                    onValueChange={(captureMode) =>
                      void patch({ captureMode: captureMode as CaptureMode })
                    }
                    items={captureModes}
                  />
                </Row>
                {settings.captureMode === 'window' ? (
                  <Row label="Window" hint="Followed by its app if the title changes">
                    <FieldSelect
                      value={settings.captureWindow}
                      placeholder="Pick a window"
                      onOpenChange={(open) => {
                        if (open) refreshWindows()
                      }}
                      onValueChange={(captureWindow) => void patch({ captureWindow })}
                      items={windowOptions(windows, settings.captureWindow)}
                    />
                  </Row>
                ) : null}
                <Row
                  label="Monitor"
                  hint={
                    settings.captureMode === 'window' ? 'Sets the frame size of clips' : undefined
                  }
                >
                  <FieldSelect
                    value={String(settings.monitorIndex)}
                    onValueChange={(value) => void patch({ monitorIndex: Number(value) })}
                    items={monitors.map((display) => ({
                      value: String(display.index),
                      label: `${display.label}${'primary' in display && display.primary ? ' (primary)' : ''}`
                    }))}
                  />
                </Row>
                <Row label="Buffer" hint="Seconds held in the replay buffer">
                  <FieldSelect
                    value={String(settings.bufferSec)}
                    onValueChange={(value) =>
                      void patch({ bufferSec: Number(value) as AppSettings['bufferSec'] })
                    }
                    items={BUFFER_SECONDS.map((seconds) => ({
                      value: String(seconds),
                      label: `${seconds}s`
                    }))}
                  />
                </Row>
                <Row label="Fallback to display" hint="Record the screen when no game is found">
                  <Switch
                    checked={settings.fallbackToDisplay}
                    onCheckedChange={(fallbackToDisplay) => void patch({ fallbackToDisplay })}
                  />
                </Row>
                {capture?.error ? (
                  <Alert variant="destructive" className="m-4 w-auto">
                    <AlertTitle>Capture</AlertTitle>
                    <AlertDescription>{capture.error}</AlertDescription>
                  </Alert>
                ) : null}
              </Rows>
            ) : null}

            {tab === 'audio' ? (
              <Rows>
                <Row label="Desktop audio" hint="Loopback device for game / system sound">
                  <FieldSelect
                    value={settings.desktopAudioId}
                    onValueChange={(desktopAudioId) => void patch({ desktopAudioId })}
                    items={deviceOptions(audioDevices.outputs, settings.desktopAudioId).map(
                      (device) => ({
                        value: device.id,
                        label: device.label
                      })
                    )}
                  />
                </Row>
                <Row label="Microphone" hint="Record your voice on its own track">
                  <Switch
                    checked={settings.micEnabled}
                    onCheckedChange={(micEnabled) => void patch({ micEnabled })}
                  />
                </Row>
                <Row label="Mic volume">
                  <div className="flex w-72 items-center gap-3">
                    <Slider
                      min={0}
                      max={200}
                      step={1}
                      value={[settings.micVolume]}
                      disabled={!settings.micEnabled}
                      onValueChange={([micVolume]) => {
                        if (micVolume === undefined) return
                        void patch({ micVolume })
                      }}
                      className="flex-1"
                    />
                    <span className="tc w-12 text-right text-[11px] text-muted-foreground">
                      {settings.micVolume}%
                    </span>
                  </div>
                </Row>
                <Row label="Mic device">
                  <FieldSelect
                    value={settings.micDeviceId}
                    onValueChange={(micDeviceId) => void patch({ micDeviceId })}
                    items={deviceOptions(audioDevices.inputs, settings.micDeviceId).map(
                      (device) => ({
                        value: device.id,
                        label: device.label
                      })
                    )}
                  />
                </Row>
              </Rows>
            ) : null}

            {tab === 'video' ? (
              <Rows>
                <Row label="Output resolution" hint="Pixel size of saved clips">
                  <FieldSelect
                    value={settings.outputRes}
                    onValueChange={(outputRes) => void patch({ outputRes: outputRes as OutputRes })}
                    items={OUTPUT_RES.map((value) => ({
                      value,
                      label: value === 'native' ? 'Native' : `${value}p`
                    }))}
                  />
                </Row>
                <Row
                  label="Visual quality"
                  hint="Detail per frame. Cloud uploads still cap at 12 Mbps"
                >
                  <FieldSelect
                    value={settings.quality}
                    onValueChange={(quality) => void patch({ quality: quality as QualityPreset })}
                    items={qualityPresets.map((value) => ({
                      value,
                      label:
                        value === 'high' ? 'High (default)' : value === 'medium' ? 'Medium' : 'Low'
                    }))}
                  />
                </Row>
                <Row label="Frame rate" hint="Frames per second encoded">
                  <FieldSelect
                    value={String(settings.fps)}
                    onValueChange={(value) =>
                      void patch({ fps: Number(value) as AppSettings['fps'] })
                    }
                    items={[
                      { value: '30', label: '30 fps' },
                      { value: '60', label: '60 fps' }
                    ]}
                  />
                </Row>
                <Row
                  label="Encoder"
                  hint={
                    capture?.encoderLabel && settings.encoderPref === 'auto'
                      ? `Using ${capture.encoderLabel}`
                      : 'Which encoder compresses the buffer'
                  }
                >
                  <FieldSelect
                    value={settings.encoderPref}
                    onValueChange={(encoderPref) =>
                      void patch({ encoderPref: encoderPref as EncoderPref })
                    }
                    items={[
                      { value: 'auto', label: 'Auto (recommended)' },
                      ...(capture?.availableEncoders ?? []).map((encoder) => ({
                        value: encoder.id,
                        label: encoder.label
                      }))
                    ]}
                  />
                </Row>
                <Row
                  label="Encoder preset"
                  hint={nvenc ? 'NVENC effort per frame' : 'Speed vs quality tradeoff'}
                >
                  <FieldSelect
                    value={settings.nvencPreset}
                    onValueChange={(nvencPreset) =>
                      void patch({ nvencPreset: nvencPreset as NvencPreset })
                    }
                    items={NVENC_PRESETS.map((value) => ({
                      value,
                      label: presetLabel(value, nvenc)
                    }))}
                  />
                </Row>
                {nvenc ? (
                  <Row
                    label="Multipass"
                    hint="NVENC analyzes frames twice. Better quality, more GPU"
                  >
                    <Switch
                      checked={settings.nvencMultipass}
                      onCheckedChange={(nvencMultipass) => void patch({ nvencMultipass })}
                    />
                  </Row>
                ) : null}
                {encoderId.includes('x264') ? (
                  <Alert className="m-4 w-auto">
                    <AlertDescription>
                      Running on x264 (CPU). Pick NVIDIA NVENC if it is listed.
                    </AlertDescription>
                  </Alert>
                ) : null}
              </Rows>
            ) : null}

            {tab === 'controls' ? (
              <Rows>
                <Row
                  label="Save clip"
                  hint={
                    hasDesktopApi() && !hotkeyRegistered
                      ? 'Registration failed, pick another combo'
                      : 'Global. Works while another app is focused'
                  }
                >
                  <div className="flex items-center gap-3">
                    {recordingHotkey ? (
                      <span className="tc animate-pulse text-[11px] font-medium text-highlight">
                        PRESS A COMBO…
                      </span>
                    ) : (
                      <Kbd className="h-7 px-2.5 text-[11.5px]">{hotkeyLabel}</Kbd>
                    )}
                    <Button
                      type="button"
                      variant={recordingHotkey ? 'default' : 'outline'}
                      onClick={() => setRecordingHotkey((value) => !value)}
                    >
                      {recordingHotkey ? 'Cancel' : 'Rebind'}
                    </Button>
                  </div>
                </Row>
              </Rows>
            ) : null}

            {tab === 'system' ? (
              <Rows>
                <Row label="Clips folder" hint="Where saved clips are written">
                  <div className="flex w-[22rem] min-w-0 items-center gap-3">
                    <Input readOnly value={settings.outputFolder} className="min-w-0 flex-1" />
                    <Button type="button" variant="outline" onClick={() => void browseOutput()}>
                      Browse
                    </Button>
                  </div>
                </Row>
                <Row label="Start with OS" hint="Launch quietly in the tray when you log in">
                  <Switch
                    checked={settings.autostart}
                    onCheckedChange={(autostart) => void patch({ autostart })}
                  />
                </Row>
              </Rows>
            ) : null}
          </div>
        </section>
      </div>
    </main>
  )
}

function FieldSelect(props: {
  value: string
  onValueChange: (value: string) => void
  items: { value: string; label: string }[]
  placeholder?: string
  onOpenChange?: (open: boolean) => void
}): React.JSX.Element {
  return (
    <Select
      value={props.value}
      onValueChange={props.onValueChange}
      onOpenChange={props.onOpenChange}
    >
      <SelectTrigger>
        <SelectValue placeholder={props.placeholder} className="truncate" />
      </SelectTrigger>
      <SelectContent>
        {props.items.map((item) => (
          <SelectItem key={item.value} value={item.value}>
            {item.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

function deviceOptions(devices: AudioDeviceInfo[], selected: string): AudioDeviceInfo[] {
  const opts: AudioDeviceInfo[] = [{ id: 'default', label: 'System default' }]
  for (const device of devices) {
    if (device.id === 'default' || opts.some((item) => item.id === device.id)) continue
    opts.push(device)
  }
  if (selected && !opts.some((item) => item.id === selected)) {
    opts.push({ id: selected, label: `${selected} (unavailable)` })
  }
  return opts
}

function windowOptions(
  windows: WindowInfo[],
  selected: string
): { value: string; label: string }[] {
  const opts = windows.map((window) => ({
    value: window.id,
    label: `${window.title} · ${window.exe}`
  }))
  if (selected && !opts.some((item) => item.value === selected)) {
    const [title = '', , exe = ''] = selected
      .split(':')
      .map((part) => part.replaceAll('#3A', ':').replaceAll('#22', '#'))
    opts.unshift({ value: selected, label: `${title || exe} (not open)` })
  }
  return opts
}

function Rows(props: { children: React.ReactNode }): React.JSX.Element {
  return <div className="stagger flex flex-col">{props.children}</div>
}

function Row(props: {
  label: string
  hint?: string
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div className="rise-in group relative flex min-h-15 items-center justify-between gap-4 border-b border-border px-5 py-2.5 transition-colors hover:bg-accent/50">
      <div className="min-w-0 pr-4">
        <p className="text-[14px] font-semibold text-foreground">{props.label}</p>
        {props.hint ? (
          <p className="mt-0.5 text-[12px] leading-4 text-muted-foreground">{props.hint}</p>
        ) : null}
      </div>
      <div className="shrink-0">{props.children}</div>
    </div>
  )
}

function presetLabel(preset: NvencPreset, nvenc: boolean): string {
  if (preset === 'p1') return nvenc ? 'Performance (P1)' : 'Faster'
  if (preset === 'p7') return nvenc ? 'Quality (P7)' : 'Better quality'
  return nvenc ? 'Balanced (P4)' : 'Balanced'
}
