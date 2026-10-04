# JunoClaw Docs (Starlight)

Source for `https://junoclaw.xyz/docs/`: the lightpaper and onboarding for validators, builders, users and partners.

```bash
npm install
npm run dev     # http://localhost:4321/docs/
npm run build   # outputs to ../website/docs (base path /docs)
```

Pages live in `src/content/docs/`, and the sidebar is configured in `astro.config.mjs`.
After a build, upload the whole `../website/` folder (landing page + `docs/`) to Cloudflare.
