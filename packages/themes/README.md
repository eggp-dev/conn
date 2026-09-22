# @conn/themes

`builtin-themes.json` is the only definition of Conn's built-in terminal palettes (background,
foreground, cursor, selection and the 16 ANSI colors). The native extension registry compiles it in
with `include_str!`, and the Svelte UI imports the same file, so the two can no longer drift.

Interface colors that are not part of a terminal palette (surfaces, muted text, status colors) stay
in `packages/ui/src/lib/themes.ts`: themes installed by users supply a palette only, and Conn
keeps control of the colors that carry approval and warning meaning.
