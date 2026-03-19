// ── Simple structured logger ──

import { CONFIG } from './config.js'

type Level = 'debug' | 'info' | 'warn' | 'error'
const LEVELS: Record<Level, number> = { debug: 0, info: 1, warn: 2, error: 3 }

const minLevel = LEVELS[CONFIG.logLevel] ?? 1

function ts(): string {
  return new Date().toISOString()
}

export function log(level: Level, component: string, message: string, data?: Record<string, unknown>) {
  if (LEVELS[level] < minLevel) return
  const prefix = `[${ts()}] [${level.toUpperCase().padEnd(5)}] [${component}]`
  const line = data ? `${prefix} ${message} ${JSON.stringify(data)}` : `${prefix} ${message}`
  if (level === 'error') console.error(line)
  else if (level === 'warn') console.warn(line)
  else console.log(line)
}

export const logger = {
  debug: (component: string, msg: string, data?: Record<string, unknown>) => log('debug', component, msg, data),
  info:  (component: string, msg: string, data?: Record<string, unknown>) => log('info', component, msg, data),
  warn:  (component: string, msg: string, data?: Record<string, unknown>) => log('warn', component, msg, data),
  error: (component: string, msg: string, data?: Record<string, unknown>) => log('error', component, msg, data),
}
