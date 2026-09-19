import { useEffect, useRef, useState } from 'react'

const prefersReducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

/**
 * Animates a number from its previous value up to `target`.
 *
 * Used on the mission stat cards so a changing metric reads as movement
 * rather than a silent swap. Jumps straight to the value when the OS asks
 * for reduced motion.
 */
export function useCountUp(target: number, durationMs = 900): number {
  const [value, setValue] = useState(() => (prefersReducedMotion() ? target : 0))
  const fromRef = useRef(value)
  const frameRef = useRef<number>()

  useEffect(() => {
    if (prefersReducedMotion()) {
      setValue(target)
      fromRef.current = target
      return
    }

    const from = fromRef.current
    const delta = target - from
    if (delta === 0) return

    const start = performance.now()

    const step = (now: number) => {
      const t = Math.min((now - start) / durationMs, 1)
      // easeOutCubic — fast start, settles gently on the final value
      const eased = 1 - Math.pow(1 - t, 3)
      const next = from + delta * eased
      setValue(next)

      if (t < 1) {
        frameRef.current = requestAnimationFrame(step)
      } else {
        fromRef.current = target
      }
    }

    frameRef.current = requestAnimationFrame(step)
    return () => {
      if (frameRef.current !== undefined) cancelAnimationFrame(frameRef.current)
      fromRef.current = value
    }
    // `value` is intentionally excluded: including it would restart the
    // animation on every frame.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target, durationMs])

  return value
}
