# victory-3060TI

Everything to run on the **NVIDIA GeForce RTX 3060 Ti (8 GB VRAM, Wave32)** — **FPS > quality** intended, while staying 100% playable.

## Git Lineage & Credits

This fork keeps full shared git history with upstream **[Marvisak/VIctory](https://github.com/Marvisak/VIctory)** so upstream fixes and PRs can be merged cleanly without unrelated-history conflicts:

`Marvisak/VIctory` -> `fxceless/victory-fork` -> [`soldsawn/victory-nvidia-8gb`](https://github.com/soldsawn/victory-nvidia-8gb) -> `victory-3060TI`

All credit for the core compatibility layer and key components belongs to:
- **[Marvisak/VIctory](https://github.com/Marvisak/VIctory)** & **[PR #3275](https://github.com/Marvisak/VIctory/pull/3275):** Core compatibility layer and lock-free thread-local view lookup cache (`LIVE` + `SEEN`) eliminating `VIEWS` `RwLock` contention across draw-recording threads.
- **[soldsawn/victory-nvidia-8gb](https://github.com/soldsawn/victory-nvidia-8gb):** Base 8 GB NVIDIA Wave32 tuning (`VICTORY_TEXTURE_DROP_MIPS`, `VICTORY_WINDOW_UPLOAD`, `VICTORY_QUEUE2_REFETCH`), ambient lighting (`VICTORY_AMBIENT`), and English performance overlay.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork) (`06702fe`, `6d20eed`, `94ed363`, [PR #1315](https://github.com/fxceless/victory-fork/pull/1315), [PR #1295](https://github.com/fxceless/victory-fork/pull/1295)) & [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX) (`46e3297`):** Flight-room tuning (`VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`), masked compute upload for high-churn pages (`VICTORY_MERGE_MASK=1`), lock-free block light refresh (`VICTORY_LIGHT_REFRESH=1`), persistent uncommitted-block tracking (`VICTORY_ABSENT_ONCE=1`, `VICTORY_ABSENT_MS=told`), unlocked `VirtualProtect` re-arming (`VICTORY_GUARD_OUTSIDE=1`), and per-pipeline `TablePlan`.
- **[p1nkman-VI](https://codeberg.org/p1nkman/p1nkman-VI):** Boat-mission null-pointer `memcpy` crash guard (`null_copy` in `victory-loader`, toggleable via `VICTORY_NULL_COPY=0`).
- **marquezini:** HID gamepad support (DualSense, DualSense Edge, DualShock 4, generic HID pads with rumble).
- **codec-d/cool_fox (`773d902`, `641835a4` & fence wakes):** Batched `Host::card_before` copies with deduplicated buffer barriers, per-pipeline `TablePlan`, and `WaitOnAddress` / `WakeByAddressAll` blocking fence waits.

## What `victory-3060TI` Adds

- **100% English codebase & launchers** with clean upstream-mergeable git history.
- **CPU / Heap-buffer & write-watch optimizations:**
  - Masked GPU compute upload (`VICTORY_MERGE_MASK=1`) for pages with >16 changed runs under in-flight shader writes, avoiding `VICTORY_MERGE_RUNS` pipeline stalls (`card_mask.rs`, `host.rs`, `shadow.rs`).
  - Per-pipeline precomputed `TablePlan` descriptor-slot grouping (`pipeline.rs`, `draw.rs`).
  - Lock-free 64 KiB block light refresh (`VICTORY_LIGHT_REFRESH=1`), `VICTORY_ABSENT_ONCE=1` / `VICTORY_ABSENT_MS=told`, and unlocked `VirtualProtect` re-arming (`VICTORY_GUARD_OUTSIDE=1`) (`watch.rs`, `shadow.rs`, `xmem.rs`).
  - Skips table-driven buffer slots (`tabled_lately`) on the 1/16th heap-buffer refresh cadence (`draw.rs`, `descriptor/host.rs`).
  - Lock-free thread-local view lookup cache (`descriptor.rs`).
  - Batched `Host::card_before` copies, deduplicated `ResourceBarrier` transitions, and single compute root-sig/PSO bind (`host.rs`).
  - `WaitOnAddress` / `WakeByAddressAll` blocking fence waits (`fence.rs`).
- **Two ready-to-use launcher presets:**
  1. **[`scripts\play-3060ti-fps.bat`](scripts/play-3060ti-fps.bat)** *(default — playable FPS maxing)*:
     - `series-s`, `VICTORY_TEXTURE_DROP_MIPS=3`, `2048/1024 MB` card pools, `VICTORY_TEXTURE_BACK=1` (water/boat missions playable), `VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`, `VICTORY_MERGE_MASK=1`, `VICTORY_LEVELS_ON_DEMAND=0`, `VICTORY_QUEUE2_REFETCH=0`, `VICTORY_WINDOW_UPLOAD=1`, `VICTORY_AMBIENT=0.6`, quiet logging by default.
     - Optional args: `warmup` (pre-translates shaders) or `warn` (verbose logging).
  2. **[`scripts\play-3060ti-quality.bat`](scripts/play-3060ti-quality.bat)** *(quality / glitch-fix)*:
     - `series-s`, `VICTORY_TEXTURE_DROP_MIPS=2` (sharper mipmaps), `VICTORY_TEXTURE_BACK=1`, `VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`, `VICTORY_MERGE_MASK=1`, `VICTORY_AMBIENT=0.6`, `warn` logging by default.
     - Optional arg: `quiet` (reduces log verbosity for long runs).

## Quick Start

1. Build the release binary:
   ```cmd
   cargo build --release
   ```
2. Set `GAME_DIR` (or edit the default `GAME_DIR=` path inside `scripts\play-3060ti-fps.bat` / `scripts\play-3060ti-quality.bat`) to point to your game directory.
3. Optional one-time shader warm-up:
   ```cmd
   scripts\play-3060ti-fps.bat warmup
   ```
4. Launch:
   ```cmd
   scripts\play-3060ti-fps.bat
   ```
   or for sharper textures / diagnostics:
   ```cmd
   scripts\play-3060ti-quality.bat
   ```

## License

GNU General Public License v3.0 (GPL-3.0), inherited from `Marvisak/VIctory` / `soldsawn/victory-nvidia-8gb`.
