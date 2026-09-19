export function Skeleton({ className = '', style }: { className?: string; style?: React.CSSProperties }) {
  return (
    <div
      className={`animate-pulse rounded-lg ${className}`}
      style={{ background: 'rgba(255,255,255,0.04)', ...style }}
    />
  )
}

export function SkeletonRow({ gap = 8 }: { gap?: number }) {
  return (
    <div className="flex items-center gap-3" style={{ marginBottom: gap }}>
      <Skeleton style={{ width: 36, height: 36, borderRadius: 8 }} />
      <div className="flex-1">
        <Skeleton style={{ width: '60%', height: 12, marginBottom: 4 }} />
        <Skeleton style={{ width: '40%', height: 10 }} />
      </div>
    </div>
  )
}

export function SkeletonCard() {
  return (
    <div className="rounded-2xl p-4" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="flex items-center gap-2.5 mb-3">
        <Skeleton style={{ width: 36, height: 36, borderRadius: 12 }} />
        <Skeleton style={{ width: 100, height: 14 }} />
      </div>
      <Skeleton style={{ width: '80%', height: 10 }} />
    </div>
  )
}

export function ScreenSkeleton() {
  return (
    <div className="flex flex-col overflow-hidden px-8 pt-8 pb-8" style={{ height: '100%' }}>
      {/* Hero skeleton */}
      <div className="flex items-center gap-4 mb-6">
        <Skeleton style={{ width: 56, height: 56, borderRadius: 16 }} />
        <div>
          <Skeleton style={{ width: 120, height: 24, marginBottom: 4 }} />
          <Skeleton style={{ width: 180, height: 14 }} />
        </div>
        <div className="ml-auto">
          <Skeleton style={{ width: 100, height: 28, borderRadius: 999 }} />
        </div>
      </div>
      {/* Stat cards skeleton */}
      <div className="grid grid-cols-4 gap-4 mb-6">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="rounded-2xl p-4" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
            <div className="flex items-center gap-2 mb-3">
              <Skeleton style={{ width: 32, height: 32, borderRadius: 8 }} />
              <Skeleton style={{ width: 80, height: 10 }} />
            </div>
            <Skeleton style={{ width: 40, height: 28 }} />
          </div>
        ))}
      </div>
      {/* Two-column skeleton */}
      <div className="grid grid-cols-2 gap-6 mb-6">
        {[0, 1].map((i) => (
          <div key={i} className="rounded-2xl p-5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
            <Skeleton style={{ width: 100, height: 14, marginBottom: 16 }} />
            {[0, 1, 2].map((j) => <SkeletonRow key={j} />)}
          </div>
        ))}
      </div>
      {/* Portal grid skeleton */}
      <Skeleton style={{ width: 80, height: 14, marginBottom: 16 }} />
      <div className="grid grid-cols-4 gap-3">
        {[0, 1, 2, 3, 4, 5, 6, 7].map((i) => (
          <SkeletonCard key={i} />
        ))}
      </div>
    </div>
  )
}
