# conn.eggp.dev

The website: a landing page (English at `/`, Korean at `/ko/`) and the documentation at `/docs`.
Astro with Starlight; static output.

```sh
npm ci
npm run dev      # http://localhost:4321
npm run build    # writes dist/
```

## What is generated, and from where

`scripts/sync.mjs` runs before `dev` and `build` and writes everything the site does not keep in git:

| Output | Source |
| --- | --- |
| `src/content/docs/` | `../docs/*.md` and `*.ko.md`. The first heading becomes the title, the GitHub language/back-link line is dropped, and relative links become site links (or GitHub links for pages the site does not publish). |
| `public/media/` | the hero film, captions and posters from `../docs/assets/` |
| `public/install.sh` | `../scripts/install.sh`, so `curl -fsSL https://conn.eggp.dev/install.sh \| sh` serves the repository's own script |
| `src/generated/release.json` | the newest published release on GitHub; without network, the version declared in the repository |

Write documentation in `../docs`, never in `src/content/docs`. Which pages are published, and in
which sidebar group, is decided in `src/site.config.mjs`; the repository address lives there too.

## Rules for the landing page

- Every sentence in `src/i18n/landing.ts` is a promise the product keeps today. Inside SSH every
  command is reviewed; locally the policy decides. Do not round that up.
- Product images are crops of the recorded take (`../media/demo/public/footage/remote/take.mp4`),
  not mock-ups. `../media/demo/RECORDING-REMOTE.md` says what was prepared.
- The star count appears from 50 stars on; below that the button stands alone.

## Hosting

Vercel project with **Root Directory** `site` and "Include files outside the Root Directory in the
Build Step" enabled (the build reads `../docs`, `../scripts` and `../frontends/tauri/package.json`).
`vercel.json` holds the rest. The site shows the release that was newest when it was built.
`.github/workflows/site-redeploy.yml` asks the host to rebuild when a release is published; it needs a
Vercel deploy hook (Project → Settings → Git → Deploy Hooks, branch `main`) saved as the repository
secret `SITE_DEPLOY_HOOK`. Without it the workflow does nothing and the site is redeployed by hand.
