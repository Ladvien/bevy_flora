# AGENTS.md

Guidance for coding agents working on `bevy_flora`. Project layout, commands and conventions are in `CLAUDE.md`; the design is in `ARCHITECTURE.md`.

## Releasing

Every release, patch releases included, does all of these in the release PR.

1. **Docs.** Add a `## [x.y.z]` entry to `CHANGELOG.md` above the previous one, with its compare link at the bottom. Update the `README.md` compatibility table and install snippet (`0.x`), the gallery scenario count in `README.md` and any `///` docs for behaviour that changed. Update every frame time quoted in `README.md`, `src/lib.rs`, `src/pool.rs` and `ARCHITECTURE.md` to the numbers in the new CHANGELOG entry. Commit the refreshed `Cargo.lock` (CI runs `cargo package --locked`).
2. **Re-record ALL THREE README clips, `docs/growth.gif`, `docs/grove.gif` and `docs/seasons.gif`. Never skip this, even when the release looks like it cannot change them.** The clips must show the code being released: the growth clip was left showing the 0.1.0 square leaves through 0.2.1. From the repo root, frames first:
   ```sh
   rm -rf target/capture/growth target/capture/grove target/capture/seasons; cargo run --release --example capture_growth -- --preset maple --out target/capture/growth; cargo run --release --example capture_growth -- --grove --out target/capture/grove; cargo run --release --example capture_growth -- --seasons --out target/capture/seasons
   ```
   then encode them:
   ```sh
   ffmpeg -y -framerate 24 -i target/capture/growth/frame_%04d.png -vf "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/growth.gif
   ffmpeg -y -framerate 24 -i target/capture/grove/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/grove.gif
   ffmpeg -y -framerate 24 -i target/capture/seasons/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/seasons.gif
   ```
   `growth.gif` is 480x360, 24 fps, 192 frames (8 s), about 5 MB; `grove.gif` and `seasons.gif` are 560x315, 20 fps, 160 frames, about 6 MB each. Keep those. The clips are deterministic (fixed clock), so on one machine a re-record of unchanged code comes out frame-identical; another GPU may differ by a level here and there. Check the results (`ffprobe` for the numbers, and look at the first, a middle and the last frame of each) and commit all three in the release PR. `docs/` is not in the crates.io package; the README embeds the clips from `raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif`, `.../grove.gif` and `.../seasons.gif`, so they go live when the PR merges to `main`.
3. **Lint and test**, all clean: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`, `cargo build --examples`, `cargo publish --dry-run --allow-dirty`.
4. **Visual QA** when anything about the look changed: the `bevy-flora-visual-qa` skill (`qa/visual_findings.md`).
5. **Version.** Bump `version` in `Cargo.toml`; `Cargo.lock` refreshes on the next build; commit it in the release PR (CI's `cargo package --locked` fails otherwise). Before 1.0 a minor bump may break the API.
6. **PR and merge.** Open a PR to `main`, wait for CI, merge with a merge commit.
7. **Publish from a clean checkout of the merged `main`**, not from a working tree with other edits:
   ```sh
   git fetch origin && git worktree add --detach ../bevy_flora-release origin/main
   cd ../bevy_flora-release && cargo publish
   ```
   Then tag the merge commit (`git tag -a vX.Y.Z -m "bevy_flora X.Y.Z" <merge-sha> && git push origin vX.Y.Z`), run `gh release create vX.Y.Z`, and `git worktree remove ../bevy_flora-release`.

`cargo` is already logged in to crates.io through `~/.cargo/credentials.toml`; no repo holds a token, so pass none and never print it.
