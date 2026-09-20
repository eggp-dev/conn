// Which of ../docs the site publishes. The repository and the site address come from @conn/brand,
// shared with the films and the release tooling.
import brand from "@conn/brand/brand.json" with { type: "json" };

export const repository = brand.repository;
export const origin = brand.site;

/** Sidebar groups. Items are file names in ../docs without `.md`; a `.ko.md` sibling becomes the Korean page. */
export const docs = [
  {
    label: "Start",
    ko: "시작하기",
    items: ["getting-started", "first-collaboration", "agent-integrations", "faq"],
  },
  {
    label: "How it works",
    ko: "동작 원리",
    items: ["security", "policy", "architecture", "shell-integration"],
  },
  {
    label: "Reference",
    ko: "레퍼런스",
    items: ["protocol", "backends", "extensions", "external-automation", "platform-support"],
  },
];
