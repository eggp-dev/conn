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
| `public/media/*-social.jpg` | JPEG copies of the social cards: several link-preview crawlers skip WebP |
| `src/generated/film.json` | the film's length, read from the MP4, for the video markup |
| `public/robots.txt` | the site address from `@conn/brand`, pointing crawlers at the sitemap |

Write documentation in `../docs`, never in `src/content/docs`. Which pages are published, and in
which sidebar group, is decided in `src/site.config.mjs`; the repository address lives there too.

## Search engines and analytics

What crawlers need is written in one place, `src/seo.mjs`, and used by both the landing pages and
the docs: link-preview tags, `hreflang` (with `x-default`), structured data for the app, the site
and the film, and the sitemap Starlight generates (`/sitemap-index.xml`, with language alternates).
Each docs page gets its search-result description from `descriptions` in `src/site.config.mjs`;
the build fails when a published page has none or when one is longer than 160 characters.

Everything that needs an account is switched on by an environment variable in the Vercel project
(Settings → Environment Variables, Production) and leaves no trace in the HTML when unset:

| Variable | What it does |
| --- | --- |
| `PUBLIC_GOOGLE_SITE_VERIFICATION` | the token from Google Search Console's "HTML tag" method. After verifying, submit `https://conn.eggp.dev/sitemap-index.xml` there. |
| `PUBLIC_NAVER_SITE_VERIFICATION` | the same for Naver Search Advisor, which matters for Korean search. |
| `PUBLIC_GA_ID` | a Google Analytics 4 measurement ID (`G-…`). Advertising signals are denied by default, and the script is not loaded at all for visitors who send Global Privacy Control or Do Not Track. |

Vercel Web Analytics needs no variable: the script is added on every Vercel build, and counting
starts once Web Analytics is enabled for the project in the Vercel dashboard (Analytics tab). It is
cookieless. Until it is enabled the script request answers 404 and nothing else happens.

The footer says that the site counts visits and that the app has no telemetry. Keep both true.

## Rules for the landing page

- Every sentence in `src/i18n/landing.ts` is a promise the product keeps today. POSIX SSH uses
  command policy without inspecting remote files on the host. Do not round that up.
- Product images are crops of the recorded take (`../media/demo/public/footage/remote/take.mp4`),
  not mock-ups. `../media/demo/RECORDING-REMOTE.md` says what was prepared.
- The star count appears from 50 stars on; below that the button stands alone.

## The scroll story

Below the film, the landing page walks through the same take beat by beat (`src/components/Story.astro`,
beats and copy in `src/story.ts`). Scrolling forward plays a beat at its recorded speed and rests on its
last frame; scrolling back goes straight to that frame. At the proposal beat the page waits: any typing
key (or a tap) plays the takeover, which is the product's own promise made with the visitor's hand.

- It is an enhancement. The script turns it on for wide screens when motion is allowed; phones,
  `prefers-reduced-motion` and no-JavaScript visitors get the still crops, and the story video is not
  downloaded for them.
- `public/story/take.mp4` is the recorded take, only resized, with a keyframe at every beat boundary.
  After a retake run `sh scripts/encode-story.sh` (needs ffmpeg), adjust the times in `src/story.ts`
  and the key list in the script together, and commit both files it writes.
- Nothing is sped up, cut inside a beat, or drawn over the footage except the key hint.

## Hosting

Vercel project with **Root Directory** `site` and "Include files outside the Root Directory in the
Build Step" enabled (the build reads `../docs`, `../scripts` and `../frontends/tauri/package.json`).
`vercel.json` holds the rest. The site shows the release that was newest when it was built.
`.github/workflows/site-redeploy.yml` asks the host to rebuild when a release is published; it needs a
Vercel deploy hook (Project → Settings → Git → Deploy Hooks, branch `main`) saved as the repository
secret `SITE_DEPLOY_HOOK`. Without it the workflow does nothing and the site is redeployed by hand.
