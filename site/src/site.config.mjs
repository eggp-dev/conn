// The one place the site names the repository and chooses which of ../docs it publishes.
// If the repository moves again, change `repository` here (and read docs/releasing.md first).
export const repository = "eggp-dev/conn";
export const origin = "https://conn.eggp.dev";

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
