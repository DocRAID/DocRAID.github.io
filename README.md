# tui_blog

Personal technical blog rendered in the browser as a [Ratatui](https://ratatui.rs/) TUI via [Ratzilla](https://github.com/orhun/ratzilla). Hosted on GitHub Pages. Posts are read from public [Notion](https://www.notion.so/) pages listed in `notion.json`.

Live: <https://docraid.github.io/>

## Run locally

Needs a Rust toolchain with the `wasm32-unknown-unknown` target and [Trunk](https://trunkrs.dev/) 0.21.x.

```bash
rustup target add wasm32-unknown-unknown
cargo run --features fetch-bin --bin fetch_content   # snapshot Notion → snapshot.json + rss.xml
trunk serve
```

`fetch_content` is optional for a first paint: if `snapshot.json` is empty the WASM app will scrape Notion in the browser once. Production builds always snapshot at deploy time so visitors do not depend on the unofficial Notion proxy.

```bash
trunk build --release
```

## Content

`notion.json` lists public Notion site URLs:

| `role` | Meaning |
| --- | --- |
| `tags` | A page whose H2 / sub-header blocks are tags; nested pages (including inside toggles) become posts |
| `about` | Optional page whose body is shown on `/about` |

Post titles may end with ` - YYYY.MM.DD` (dots, dashes, or slashes). That date is used for “recent posts” sorting and the list label.

Rebuild the snapshot after editing Notion or `notion.json`.

## Keyboard

| Key | Action |
| --- | --- |
| `j` / `k` or arrows | Scroll, or move the blog list |
| `d` / `u` or PageDown / PageUp | Jump |
| `g` / `G` or Home / End | Top / bottom |
| Enter | Open the selected list row |
| Esc / `q` / Backspace | Back |
| `/` | Filter posts (on the blog list) |
| Tab / Shift+Tab | Cycle Intro / Blog / About |
| `1` `2` `3` | Intro / Blog / About |

On a phone, drag to scroll. Tapping still follows hover/click targets.

## Deploy

Pushes to `main` run format, clippy, and tests, snapshot Notion, build with Trunk, copy `index.html` to `404.html` (so `/about` and `/blog/...` work on GitHub Pages), and publish.

## License

Copyright (c) DocRAID <l01062506145@gmail.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

The bundled Fira Code Regular font is licensed under the SIL Open Font License 1.1.

[LICENSE]: ./LICENSE
