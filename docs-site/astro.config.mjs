import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// Builds straight into ../website/docs so the whole site (landing + docs)
// is one folder for Cloudflare upload. Served at https://junoclaw.xyz/docs/
export default defineConfig({
  site: 'https://junoclaw.xyz',
  base: '/docs',
  outDir: '../website/docs',
  trailingSlash: 'ignore',
  integrations: [
    starlight({
      title: 'JunoClaw Docs',
      description:
        'Lightpaper and onboarding for JunoClaw — the post-quantum finality chain for verifiable agents.',
      logo: { src: './src/assets/logo.png', alt: 'JunoClaw' },
      favicon: '/favicon.png',
      customCss: ['./src/styles/junoclaw.css'],
      social: [
        { icon: 'telegram', label: 'Telegram', href: 'https://t.me/junoclaw' },
        { icon: 'x.com', label: 'X / Twitter', href: 'https://twitter.com/junoclawdao' },
        { icon: 'github', label: 'GitHub', href: 'https://github.com/Dragonmonk111/junoclaw' },
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Lightpaper', slug: 'lightpaper' },
            { label: 'Status & roadmap', slug: 'status' },
          ],
        },
        {
          label: 'Onboarding',
          items: [
            { label: 'Validators — G1 call', slug: 'onboarding/validators' },
            { label: 'Builders', slug: 'onboarding/builders' },
            { label: 'Users & agents', slug: 'onboarding/users' },
            { label: 'Investors & partners', slug: 'onboarding/investors' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Run a node', slug: 'reference/run-a-node' },
            { label: 'Security model', slug: 'reference/security' },
            { label: 'FAQ', slug: 'reference/faq' },
          ],
        },
      ],
    }),
  ],
});
