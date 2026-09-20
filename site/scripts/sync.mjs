// Generates what the site does not keep in git: the docs pages (from ../docs, the single source
// of truth), the films, and the current release. Runs before `dev` and `build`.
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { descriptions, docs, origin, repository } from "../src/site.config.mjs";

const site = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(site, "..");
const blob = `https://github.com/${repository}/blob/main`;
const raw = `https://raw.githubusercontent.com/${repository}/main`;
const published = new Set(docs.flatMap((group) => group.items));

/** Where a relative link inside docs/ should point once the page lives on the site. */
function rewrite(target, locale) {
  if (/^([a-z]+:|#|\/)/i.test(target)) return target;
  const [path, hash = ""] = target.split("#");
  const anchor = hash ? `#${hash}` : "";
  const name = path.replace(/^\.\//, "");
  const page = name.match(/^([A-Za-z0-9_-]+?)(\.ko)?\.md$/);
  if (page && published.has(page[1])) {
    const prefix = page[2] || (locale === "ko" && existsSync(join(repo, "docs", `${page[1]}.ko.md`))) ? "/ko" : "";
    return `${prefix}/docs/${page[1].toLowerCase()}/${anchor}`;
  }
  const resolved = join("docs", name).replaceAll("\\", "/");
  if (/\.(png|jpe?g|webp|gif|svg|mp4|vtt|srt)$/i.test(name)) return `${raw}/${resolved}`;
  return `${blob}/${resolved}${anchor}`;
}

/** What a search result shows under the title. Written by hand in site.config.mjs: a page without one fails the build. */
function describe(name, locale) {
  const text = descriptions[name]?.[locale];
  if (!text) throw new Error(`site.config.mjs has no ${locale} description for docs/${name}`);
  if (text.length > 160) throw new Error(`the ${locale} description for docs/${name} is ${text.length} characters; search results cut it at about 160`);
  return text;
}

/** Title from the first heading; the language/back-link line under it belongs to GitHub, not the site. */
function page(source, locale, name) {
  const lines = source.split("\n");
  const at = lines.findIndex((line) => line.startsWith("# "));
  if (at < 0) throw new Error("a published doc needs a first-level heading");
  const title = lines[at].slice(2).trim();
  let rest = lines.slice(at + 1);
  const first = rest.findIndex((line) => line.trim() !== "");
  if (first >= 0 && /\]\(\.\.\/README(\.ko)?\.md\)|^(English ·|\[English\]\()/.test(rest[first])) rest.splice(first, 1);
  const body = rest
    .join("\n")
    .replace(/(\]\()([^)\s]+)(\))/g, (_, open, target, close) => open + rewrite(target, locale) + close)
    .replace(/\b(src|href|poster)="([^"]+)"/g, (_, attr, target) => `${attr}="${rewrite(target, locale)}"`);
  return `---\ntitle: ${JSON.stringify(title)}\ndescription: ${JSON.stringify(describe(name, locale))}\neditUrl: false\n---\n${body}`;
}

const out = join(site, "src", "content", "docs");
rmSync(out, { recursive: true, force: true });
let pages = 0;
for (const name of published) {
  for (const [locale, file] of [["en", `${name}.md`], ["ko", `${name}.ko.md`]]) {
    const from = join(repo, "docs", file);
    if (!existsSync(from)) continue;
    const to = join(out, locale === "ko" ? "ko" : "", "docs", `${name.toLowerCase()}.md`);
    mkdirSync(dirname(to), { recursive: true });
    writeFileSync(to, page(readFileSync(from, "utf8"), locale, name));
    pages += 1;
  }
}

const media = join(site, "public", "media");
rmSync(media, { recursive: true, force: true });
mkdirSync(media, { recursive: true });
for (const lang of ["en", "ko"]) {
  for (const suffix of [".mp4", ".vtt", "-poster.webp", "-social.webp"]) {
    copyFileSync(join(repo, "docs", "assets", `conn-remote-${lang}${suffix}`), join(media, `conn-remote-${lang}${suffix}`));
  }
  // Link previews: several crawlers skip WebP, so the social card is also published as JPEG.
  await sharp(join(media, `conn-remote-${lang}-social.webp`)).jpeg({ quality: 86, mozjpeg: true }).toFile(join(media, `conn-remote-${lang}-social.jpg`));
}

/** Seconds of an MP4, from its movie header, for the video markup on the landing pages. */
function seconds(file) {
  const bytes = readFileSync(file);
  const at = bytes.indexOf("mvhd", 0, "latin1");
  if (at < 0) throw new Error(`${file} has no movie header`);
  const wide = bytes[at + 4] === 1;
  const timescale = bytes.readUInt32BE(at + (wide ? 24 : 16));
  const duration = wide ? Number(bytes.readBigUInt64BE(at + 28)) : bytes.readUInt32BE(at + 20);
  return Math.round(duration / timescale);
}
const film = Object.fromEntries(["en", "ko"].map((lang) => [lang, { seconds: seconds(join(media, `conn-remote-${lang}.mp4`)) }]));

writeFileSync(join(site, "public", "robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${origin}/sitemap-index.xml\n`);

// `curl -fsSL https://conn.eggp.dev/install.sh | sh` serves the repository's own script.
copyFileSync(join(repo, "scripts", "install.sh"), join(site, "public", "install.sh"));

// The release the install section links to. Asking GitHub keeps the links valid between a version
// bump on main and the release being published; without network the repository's version is used.
const declared = JSON.parse(readFileSync(join(repo, "frontends", "tauri", "package.json"), "utf8")).version;
let version = declared;
try {
  const response = await fetch(`https://api.github.com/repos/${repository}/releases?per_page=20`, {
    headers: { accept: "application/vnd.github+json", "user-agent": "conn-site" },
    signal: AbortSignal.timeout(8000),
  });
  if (response.ok) {
    const live = (await response.json()).find((release) => !release.draft && /^v\d+\.\d+\.\d+$/.test(release.tag_name));
    if (live) version = live.tag_name.slice(1);
  }
} catch {
  // offline build: fall through to the declared version
}
mkdirSync(join(site, "src", "generated"), { recursive: true });
writeFileSync(join(site, "src", "generated", "release.json"), `${JSON.stringify({ version }, null, 2)}\n`);
writeFileSync(join(site, "src", "generated", "film.json"), `${JSON.stringify(film, null, 2)}\n`);

console.log(`sync: ${pages} docs pages, films copied, release v${version}`);
