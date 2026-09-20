import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import { headEntries, socialImage } from "./src/seo.mjs";
import { docs, origin, repository } from "./src/site.config.mjs";

const card = socialImage("en");

export default defineConfig({
  site: origin,
  integrations: [
    starlight({
      title: "Conn",
      description: "Your agent works in your terminal. You keep the keyboard.",
      logo: { src: "./src/assets/conn-mark.svg", alt: "" },
      favicon: "/favicon.svg",
      defaultLocale: "root",
      locales: {
        root: { label: "English", lang: "en" },
        ko: { label: "한국어", lang: "ko" },
      },
      social: [{ icon: "github", label: "GitHub", href: `https://github.com/${repository}` }],
      customCss: ["./src/styles/docs.css"],
      // Starlight writes the title, description, canonical and hreflang links itself; these are
      // the link-preview image and the verification and analytics tags shared with the landing pages.
      head: [
        { tag: "meta", attrs: { name: "robots", content: "index, follow, max-image-preview:large" } },
        { tag: "meta", attrs: { property: "og:image", content: card.url } },
        { tag: "meta", attrs: { property: "og:image:width", content: String(card.width) } },
        { tag: "meta", attrs: { property: "og:image:height", content: String(card.height) } },
        { tag: "meta", attrs: { name: "twitter:image", content: card.url } },
        ...headEntries(),
      ],
      sidebar: docs.map((group) => ({
        label: group.label,
        translations: { ko: group.ko },
        items: group.items.map((name) => `docs/${name.toLowerCase()}`),
      })),
      // The landing pages are this site's own (src/pages); Starlight only owns /docs.
      disable404Route: false,
      pagefind: true,
    }),
  ],
});
