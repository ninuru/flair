# AGENTS.md

Guidance for agents working in this repository.

## Purpose

flair is a minimal GPU terminal emulator for Wayland (tested on Hyprland) built around a tmux-style
session server: a daemon owns every session, window, pane and PTY, and a thin GPU client draws
cells and forwards input. It speaks tmux vocabulary (sessions, windows, panes, prefix, detach,
attach) and deliberately has no AI, no cloud, no login, no telemetry and no blocks. X11, macOS and
Windows are non-goals.

## Commands

- `cargo build` - build the workspace
- `cargo run` - build and run flair
- `cargo test --workspace` - run all tests (`cargo nextest run --workspace` when available)
- `cargo test -p <crate>` - run the tests of one crate
- `cargo clippy --workspace --all-targets -- -D warnings` - lint
- `cargo fmt --all` - format
- `script/install` - install a release build into `~/.cargo/bin`; a running server keeps running
- `FLAIR_STARTUP_PROBE=1 flair` - print startup phase timings up to the first frame
- `RUST_LOG=flair=debug flair` - verbose logging; the server logs to `~/.local/state/flair/server.log`

## Validation Order

Optimize for fast delivery and let CI catch what targeted local runs miss.

1. While editing, run only the smallest targeted `cargo check -p <crate>` or test that gives useful
   feedback. Defer the rest until the implementation is complete.
2. Once the code and self-review are done, run the relevant tests and fix until they pass.
3. Run Clippy with `-D warnings` and fix its findings. Rerun tests only when a fix changes behavior.
4. Run `cargo fmt --all` once, after all other changes.
5. Do not rerun tests or lint after formatting, and do not repeat validation on an unchanged
   candidate. CI runs check, test, clippy and fmt on every PR.

Do not run heavy builds in parallel.

## Crate Map

| Crate | Role |
|---|---|
| `flair-core` | pure logic and protocol types: ids, geometry, keys, commands, config, theme, wire protocol and codec, session tree, client views, layout, prefix machine |
| `flair-term` | wrapper around `alacritty_terminal`: terminal state, snapshots to wire rows, OSC 7/133 sniffer, key/paste/mouse encoder, PTY spawn |
| `flair-server` | daemon state machine (events in, effects out), runtime threads, socket listener |
| `flair-gui` | client: winit app, input translation, mirror, compose to `CellGrid`, fonts, glyph atlas, GL renderer, clipboard |
| `flair` | the single binary: CLI, autostart, wiring |

Dependency direction:

```
flair ──> flair-gui ──────────────> flair-core
  └────> flair-server ──> flair-term ──> flair-core
```

- `flair-core` has no I/O except reading the config file, no clock (time is passed in) and no
  threads.
- The daemon side (`flair-server`, `flair-term`) never links winit, glutin, glow, crossfont or
  clipboard crates. The `flair server` code path never initializes winit or GL.
- The client side (`flair-gui`) never links `flair-term` or `alacritty_terminal`.
- All GL code lives in `flair-gui/src/renderer/`. The renderer boundary is `CellGrid` in, pixels out.

## Coding Style

- If a function takes a context parameter, name it `ctx` and put it last, unless a closure
  parameter takes the last place.
- Remove unused parameters completely and update all call sites; never prefix them with `_`.
- Use inline format arguments: `format!("{name}")`, not `format!("{}", name)`.
- Put imports at the top of the file and prefer them over long path qualifiers.
- Match exhaustively on flair's own enums. No `_` arms.
- Avoid unnecessary type annotations, especially on closure parameters.
- Do not add abstractions, traits or configurability for a single use.

## Comments

- No code comments: no inline, block or narration comments in any file.
- Doc comments only on public API where the name and signature are not enough.
- No transformation comments that describe an edit; explanations of changes belong in the PR.
- Leave existing comments alone unless the code they describe changes.
- State this rule in every delegated implementation task.

## Tests

- Unit tests live in `<file>_tests.rs`, included at the end of the module:

  ```rust
  #[cfg(test)]
  #[path = "server_tests.rs"]
  mod tests;
  ```

- Tests are hermetic: synthetic key events, fake PTYs or effects-only server tests
  (`Server::handle` in, `Effect`s out, no threads, no clock), render-to-cells for the UI.
- Every test run or manual app verification that starts or connects to a daemon must set a unique
  `FLAIR_SOCKET` path inside a private temporary directory (mode 0700). Allocate a fresh path per
  worktree and concurrent test run, and pass it to every related client, daemon and cleanup command.
  Use only that socket for teardown; never test against the user's regular daemon or inherited
  `FLAIR` socket. Stop the test daemon and remove its temporary directory when finished.
- Never inject keys through the compositor.
- Tests that need a GPU, EGL device or shell say so and skip cleanly with a log line.
- No coverage percentage target.

## Dependencies

- `alacritty_terminal`, `winit`, `glutin` and `glow` are pinned exactly (`=x.y.z`) in
  `[workspace.dependencies]`.
- `cargo update` runs only as its own deliberate commit.
- Keep the dependency set small. A new crate needs a one-line justification in the blueprint.
- flair is MIT. Never copy Warp code (AGPL).

## Workflow

- Blueprint before code: every milestone starts from an approved plan. When the design changes,
  update the plan first.
- One branch per issue, PR to `main`, merged with a merge commit.
- The user pushes and closes issues by hand. Do not push.
- At the end of each milestone, hand one release build to the user, installed with `script/install`,
  together with the acceptance list to try.

## Local Docs

- `docs/` (blueprint, roadmap, milestone plans) and `.claude/` are gitignored local planning files.
  Never commit them.
- Read `.claude/HANDOFF.md` first when it exists.
