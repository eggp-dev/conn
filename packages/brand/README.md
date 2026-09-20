# @conn/brand

The one place that says what Conn is called and where it lives. `brand.json` is plain data so every
toolchain in the repository can read it: the site and the films import it, and the release tooling
loads it with `json`.

If the repository moves again, change `repository` here and read
[the release guide](../../docs/releasing.md) first: the in-app updater keeps its own literal list of
trusted addresses on purpose, and `previousRepository` must keep working for installed builds.
