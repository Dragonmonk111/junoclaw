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
      },
      keyframes: {
        fadeIn:  { from: { opacity: '0' }, to: { opacity: '1' } },
        slideUp: { from: { opacity: '0', transform: 'translateY(6px)' }, to: { opacity: '1', transform: 'translateY(0)' } },
      },
    },
  },
  plugins: [],
}
