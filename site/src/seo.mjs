// What search engines, link previews and the two analytics services need, in one place so the
// landing pages (src/components/Landing.astro) and the docs (Starlight, astro.config.mjs) agree.
// Everything optional is switched on by an environment variable at build time; see site/README.md.
import { origin } from "./site.config.mjs";

const env = (name) => (process.env[name] ?? "").trim();
const googleAnalytics = /^G-[A-Z0-9]{4,}$/.test(env("PUBLIC_GA_ID")) ? env("PUBLIC_GA_ID") : "";
const token = (name) => (/^[A-Za-z0-9_-]{8,}$/.test(env(name)) ? env(name) : "");

/** Link previews want JPEG or PNG; several crawlers (KakaoTalk, LinkedIn, Slack) skip WebP. */
export const socialImage = (lang) => ({
  url: new URL(`/media/conn-remote-${lang}-social.jpg`, origin).href,
  width: 1280,
  height: 720,
});

/** Head entries in Starlight's shape: { tag, attrs, content }. */
export function headEntries() {
  const entries = [];
  for (const [name, variable] of [["google-site-verification", "PUBLIC_GOOGLE_SITE_VERIFICATION"], ["naver-site-verification", "PUBLIC_NAVER_SITE_VERIFICATION"]]) {
    if (token(variable)) entries.push({ tag: "meta", attrs: { name, content: token(variable) } });
  }
  // Vercel Web Analytics: cookieless, served by the deployment itself, so only on Vercel builds.
  if (env("VERCEL")) {
    entries.push({ tag: "script", content: "window.va=window.va||function(){(window.vaq=window.vaq||[]).push(arguments)};" });
    entries.push({ tag: "script", attrs: { defer: true, src: "/_vercel/insights/script.js" } });
  }
  // Google Analytics 4: measurement only (advertising signals denied), and not loaded at all for
  // visitors who send Global Privacy Control or Do Not Track.
  if (googleAnalytics) {
    entries.push({
      tag: "script",
      content:
        `if(!(navigator.globalPrivacyControl||navigator.doNotTrack==="1")){window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments)}` +
        `gtag("consent","default",{ad_storage:"denied",ad_user_data:"denied",ad_personalization:"denied",analytics_storage:"granted"});` +
        `gtag("js",new Date());gtag("config","${googleAnalytics}");` +
        `var s=document.createElement("script");s.async=true;s.src="https://www.googletagmanager.com/gtag/js?id=${googleAnalytics}";document.head.appendChild(s)}`,
    });
  }
  return entries;
}

const escape = (value) => String(value).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/** The same entries as markup, for pages that write their own <head>. */
export function headHtml() {
  return headEntries()
    .map(({ tag, attrs = {}, content }) => {
      const rendered = Object.entries(attrs).map(([key, value]) => (value === true ? ` ${key}` : ` ${key}="${escape(value)}"`)).join("");
      return tag === "meta" ? `<meta${rendered}>` : `<${tag}${rendered}>${content ?? ""}</${tag}>`;
    })
    .join("");
}

/** JSON-LD for a script tag: "<" must not be able to close the element. */
export const jsonLd = (data) => JSON.stringify(data).replace(/</g, "\\u003c");
