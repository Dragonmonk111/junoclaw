/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        juno: {
          50:  '#fff3ef',
          100: '#ffe4da',
          200: '#ffcab5',
          300: '#ffa480',
          400: '#ff7a52',
          500: '#ff6b4a',
          600: '#e84e2c',
          700: '#c43820',
          800: '#9e2f1d',
          900: '#7f2a1c',
          950: '#45120a',
        },
        teal: {
          400: '#2dd4bf',
          500: '#00d4aa',
          600: '#00b890',
        },
        void: {
          950: '#06060f',
          900: '#0a0a18',
          800: '#0f0f20',
          700: '#16162b',
          600: '#1e1e38',
          500: '#2a2a4a',
        },
      },
      boxShadow: {
        'glow-juno':  '0 0 20px rgba(255,107,74,0.25)',
        'glow-teal':  '0 0 20px rgba(0,212,170,0.20)',
        'glow-sm':    '0 0 8px  rgba(255,107,74,0.35)',
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4,0,0.6,1) infinite',
        'fade-in':    'fadeIn 0.3s ease-out',
        'slide-up':   'slideUp 0.25s ease-out',
        'rise':       'rise 0.45s cubic-bezier(0.22,1,0.36,1) both',
        'radar':      'radar 2.4s cubic-bezier(0,0,0.2,1) infinite',
        'sweep':      'sweep 3.5s ease-in-out infinite',
        'breathe':    'breathe 4s ease-in-out infinite',
        'tick':       'tick 1.6s ease-in-out infinite',
      },
      keyframes: {
        fadeIn:  { from: { opacity: '0' }, to: { opacity: '1' } },
        slideUp: { from: { opacity: '0', transform: 'translateY(6px)' }, to: { opacity: '1', transform: 'translateY(0)' } },
        rise: {
          from: { opacity: '0', transform: 'translateY(10px) scale(0.985)' },
          to:   { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
        // Radar ping — expands outward from a live status dot
        radar: {
          '0%':   { transform: 'scale(1)',   opacity: '0.55' },
          '70%':  { transform: 'scale(2.8)', opacity: '0' },
          '100%': { transform: 'scale(2.8)', opacity: '0' },
        },
        // Light sweep across a surface, used for the claim CTA
        sweep: {
          '0%':        { transform: 'translateX(-120%)' },
          '55%, 100%': { transform: 'translateX(320%)' },
        },
        // Slow scale/opacity breath for ambient glows
        breathe: {
          '0%, 100%': { opacity: '0.35', transform: 'scale(1)' },
          '50%':      { opacity: '0.7',  transform: 'scale(1.06)' },
        },
        // Discrete step, mirrors a reflex cycle firing
        tick: {
          '0%, 100%': { opacity: '0.25' },
          '50%':      { opacity: '1' },
        },
      },
    },
  },
  plugins: [],
}
