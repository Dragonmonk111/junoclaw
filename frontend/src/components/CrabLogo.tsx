export function CrabLogo({ size = 96, className = '' }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 120 120"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <defs>
        <linearGradient id="shell-main" x1="30%" y1="0%" x2="70%" y2="100%">
          <stop offset="0%" stopColor="#ffab91" />
          <stop offset="40%" stopColor="#ff6b4a" />
          <stop offset="100%" stopColor="#bf360c" />
        </linearGradient>
        <linearGradient id="shell-dark" x1="50%" y1="0%" x2="50%" y2="100%">
          <stop offset="0%" stopColor="#e84e2c" />
          <stop offset="100%" stopColor="#8b2500" />
        </linearGradient>
        <linearGradient id="claw-main" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#ff8a65" />
          <stop offset="100%" stopColor="#d84315" />
        </linearGradient>
        <linearGradient id="circuit-glow" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stopColor="#ff6b4a" stopOpacity="0" />
          <stop offset="50%" stopColor="#ff6b4a" stopOpacity="0.6" />
          <stop offset="100%" stopColor="#ff6b4a" stopOpacity="0" />
        </linearGradient>
        <filter id="geo-glow" x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="2" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {/* === LEFT CLAW — raised up (attack) === */}
      <g filter="url(#geo-glow)">
        {/* Arm segments — angular */}
        <polygon points="34,54 28,42 32,40 38,50" fill="url(#claw-main)" />
        <polygon points="28,42 22,28 26,26 32,40" fill="url(#claw-main)" />
        {/* Pincer top — angular blade */}
        <polygon points="22,28 10,14 14,12 24,24" fill="#ff7043" />
        <polygon points="14,12 8,8 10,6 16,10" fill="#ffab91" />
        {/* Pincer bottom */}
        <polygon points="22,28 12,22 14,18 24,24" fill="#e65100" />
        {/* Joint accent */}
        <circle cx="23" cy="27" r="2" fill="#bf360c" stroke="#ff8a65" strokeWidth="0.8" />
      </g>

      {/* === RIGHT CLAW — lowered (defend) === */}
      <g filter="url(#geo-glow)">
        {/* Arm segments */}
        <polygon points="86,54 92,48 96,50 90,56" fill="url(#claw-main)" />
        <polygon points="92,48 100,56 104,54 96,46" fill="url(#claw-main)" />
        {/* Pincer top */}
        <polygon points="100,56 112,62 110,66 98,60" fill="#ff7043" />
        <polygon points="112,62 118,64 116,68 110,66" fill="#ffab91" />
        {/* Pincer bottom */}
        <polygon points="100,56 110,68 108,72 98,60" fill="#e65100" />
        {/* Joint accent */}
        <circle cx="99" cy="56" r="2" fill="#bf360c" stroke="#ff8a65" strokeWidth="0.8" />
      </g>

      {/* === LEGS LEFT — 3 angular segments === */}
      <g fill="none" strokeLinejoin="bevel">
        <polyline points="36,70 24,76 18,86 22,90" stroke="#d84315" strokeWidth="2" />
        <polyline points="34,74 22,82 16,94 20,97" stroke="#bf360c" strokeWidth="1.8" />
        <polyline points="34,78 24,90 20,102 24,104" stroke="#8b2500" strokeWidth="1.5" />
      </g>

      {/* === LEGS RIGHT === */}
      <g fill="none" strokeLinejoin="bevel">
        <polyline points="84,70 96,76 102,86 98,90" stroke="#d84315" strokeWidth="2" />
        <polyline points="86,74 98,82 104,94 100,97" stroke="#bf360c" strokeWidth="1.8" />
        <polyline points="86,78 96,90 100,102 96,104" stroke="#8b2500" strokeWidth="1.5" />
      </g>

      {/* === MAIN SHELL — faceted gem shape === */}
      {/* Base wide hex */}
      <polygon points="60,42 84,50 90,66 82,82 38,82 30,66 36,50"
               fill="url(#shell-main)" />
      {/* Top facet — lighter */}
      <polygon points="60,42 84,50 74,56 46,56 36,50"
               fill="#ffab91" opacity="0.5" />
      {/* Left facet */}
      <polygon points="36,50 46,56 40,72 30,66"
               fill="url(#shell-dark)" opacity="0.4" />
      {/* Right facet */}
      <polygon points="84,50 74,56 80,72 90,66"
               fill="url(#shell-dark)" opacity="0.3" />
      {/* Bottom facet */}
      <polygon points="46,56 74,56 80,72 82,82 38,82 40,72"
               fill="url(#shell-main)" opacity="0.85" />

      {/* === FACET EDGES — sharp geometric lines === */}
      <g stroke="#bf360c" strokeWidth="0.6" fill="none" opacity="0.6">
        <line x1="46" y1="56" x2="40" y2="72" />
        <line x1="74" y1="56" x2="80" y2="72" />
        <line x1="46" y1="56" x2="74" y2="56" />
        <line x1="40" y1="72" x2="80" y2="72" />
        <line x1="60" y1="42" x2="60" y2="56" />
        <line x1="60" y1="56" x2="60" y2="82" />
        <line x1="60" y1="56" x2="40" y2="72" />
        <line x1="60" y1="56" x2="80" y2="72" />
      </g>

      {/* === CIRCUIT TRACES on shell === */}
      <g stroke="#ff6b4a" strokeWidth="0.5" fill="none" opacity="0.35">
        {/* Horizontal traces */}
        <line x1="44" y1="62" x2="56" y2="62" />
        <line x1="64" y1="62" x2="76" y2="62" />
        <line x1="42" y1="68" x2="52" y2="68" />
        <line x1="68" y1="68" x2="78" y2="68" />
        {/* Vertical traces */}
        <line x1="52" y1="58" x2="52" y2="68" />
        <line x1="68" y1="58" x2="68" y2="68" />
        {/* Nodes */}
      </g>
      <g fill="#ff6b4a" opacity="0.4">
        <circle cx="52" cy="62" r="1" />
        <circle cx="68" cy="62" r="1" />
        <circle cx="52" cy="68" r="1" />
        <circle cx="68" cy="68" r="1" />
        <circle cx="60" cy="65" r="1.2" />
      </g>

      {/* === EYE STALKS — angular === */}
      <g filter="url(#geo-glow)">
        {/* Left eye */}
        <polygon points="48,50 44,40 46,38 50,48" fill="#e84e2c" />
        <polygon points="40,36 44,40 48,38 44,34" fill="#1a1a2e" stroke="#ff6b4a" strokeWidth="1" />
        <circle cx="44" cy="36.5" r="2" fill="#00d4aa" />
        <circle cx="43.5" cy="36" r="0.7" fill="#fff" opacity="0.9" />

        {/* Right eye */}
        <polygon points="72,50 76,40 74,38 70,48" fill="#e84e2c" />
        <polygon points="80,36 76,40 72,38 76,34" fill="#1a1a2e" stroke="#ff6b4a" strokeWidth="1" />
        <circle cx="76" cy="36.5" r="2" fill="#00d4aa" />
        <circle cx="75.5" cy="36" r="0.7" fill="#fff" opacity="0.9" />
      </g>

      {/* === SHELL HIGHLIGHT — gem specular === */}
      <polygon points="50,48 60,45 66,50 56,52" fill="rgba(255,255,255,0.15)" />
      <polygon points="62,58 70,56 72,60 64,62" fill="rgba(255,255,255,0.06)" />

      {/* === BOTTOM EDGE — shadow === */}
      <line x1="38" y1="82" x2="82" y2="82" stroke="#4a1a08" strokeWidth="1.5" opacity="0.4" />
    </svg>
  )
}
