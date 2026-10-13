const CODE_KEYS: Record<string, string> = {
  Space: 'Space',
  Tab: 'Tab',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backquote: '`'
}

export function eventToAccelerator(event: KeyboardEvent, platform: NodeJS.Platform): string | null {
  const key = keyFromEvent(event)
  if (!key) return null

  const parts: string[] = []
  if (event.ctrlKey) parts.push('Control')
  if (event.metaKey) parts.push(platform === 'darwin' ? 'Command' : 'Super')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')
  parts.push(key)

  const modifierCount = parts.length - 1
  const isFunctionKey = /^F\d{1,2}$/.test(key)
  if (modifierCount === 0 && !isFunctionKey) return null

  return parts.join('+')
}

function keyFromEvent(event: KeyboardEvent): string | null {
  if (
    event.key === 'Shift' ||
    event.key === 'Control' ||
    event.key === 'Alt' ||
    event.key === 'Meta'
  ) {
    return null
  }

  const { code } = event
  const digit = /^Digit([0-9])$/.exec(code)
  if (digit) return digit[1]
  const letter = /^Key([A-Z])$/.exec(code)
  if (letter) return letter[1]
  const fn = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code)
  if (fn) return code
  if (CODE_KEYS[code]) return CODE_KEYS[code]
  if (code.startsWith('Numpad')) {
    const suffix = code.slice('Numpad'.length)
    return `num${suffix.toLowerCase()}`
  }
  if (event.key.length === 1) return event.key.toUpperCase()
  return null
}
