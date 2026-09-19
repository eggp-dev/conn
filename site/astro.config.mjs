import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import { docs, origin, repository } from "./src/site.config.mjs";

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
