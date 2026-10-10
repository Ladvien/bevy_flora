# AGENTS.md

Guidance for coding agents working on `bevy_flora`. Project layout, commands and conventions are in `CLAUDE.md`; the design is in `ARCHITECTURE.md`.

## Releasing

Every release, patch releases included, does all of these in the release PR.

1. **Docs.** Add a `## [x.y.z]` entry to `CHANGELOG.md` above the previous one, with its compare link at the bottom. Update the `README.md` compatibility table and install snippet (`0.x`), the gallery scenario count in `README.md` and any `///` docs for behaviour that changed.
2. **Re-record the README clip, `docs/growth.gif`. Never skip this, even when the release looks like it cannot change the oak.** The clip must show the code being released: it was left showing the 0.1.0 square leaves through 0.2.1. From the repo root:
   ```sh
   rm -rf target/capture/growth
   cargo run --release --example capture_growth -- --out target/capture/growth
   ffmpeg -y -framerate 24 -i target/capture/growth/frame_%04d.png \
     -vf "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" docs/growth.gif
   ```
   The old clip is 480x360, 24 fps, 192 frames (8 s). Keep that, check the result (`ffprobe` for the numbers, and look at the first, a middle and the last frame), and commit `docs/growth.gif` in the release PR. `docs/` is not in the crates.io package; the README embeds the clip from `raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif`, so it goes live when the PR merges to `main`.
3. **Lint and test**, all clean: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`, `cargo build --examples`, `cargo publish --dry-run --allow-dirty`.
4. **Visual QA** when anything about the look changed: the `bevy-flora-visual-qa` skill (`qa/visual_findings.md`).
5. **Version.** Bump `version` in `Cargo.toml`; `Cargo.lock` refreshes on the next build. Before 1.0 a minor bump may break the API.
6. **PR and merge.** Open a PR to `main`, wait for CI, merge with a merge commit.
7. **Publish from a clean checkout of the merged `main`**, not from a working tree with other edits:
   ```sh
   git fetch origin && git worktree add --detach ../bevy_flora-release origin/main
   cd ../bevy_flora-release && cargo publish
   ```
   Then tag the merge commit (`git tag -a vX.Y.Z -m "bevy_flora X.Y.Z" <merge-sha> && git push origin vX.Y.Z`), run `gh release create vX.Y.Z`, and `git worktree remove ../bevy_flora-release`.

`cargo` is already logged in to crates.io through `~/.cargo/credentials.toml`; no repo holds a token, so pass none and never print it.
