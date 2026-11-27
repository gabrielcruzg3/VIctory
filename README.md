# victory-3060TI

Everything to run on the **NVIDIA GeForce RTX 3060 Ti (8 GB VRAM, Wave32)** — **FPS > quality** intended, while staying 100% playable.

## Git Lineage & Credits

This fork keeps full shared git history with upstream **[Marvisak/VIctory](https://github.com/Marvisak/VIctory)** so upstream fixes and PRs can be merged cleanly without unrelated-history conflicts:

`Marvisak/VIctory` -> `fxceless/victory-fork` -> [`soldsawn/victory-nvidia-8gb`](https://github.com/soldsawn/victory-nvidia-8gb) -> `victory-3060TI`

All credit for the core compatibility layer and key components belongs to:
- **[Marvisak/VIctory](https://github.com/Marvisak/VIctory)** & **[PR #3275](https://github.com/Marvisak/VIctory/pull/3275):** Core compatibility layer and lock-free thread-local view lookup cache (`LIVE` + `SEEN`) eliminating `VIEWS` `RwLock` contention across draw-recording threads.
- **[soldsawn/victory-nvidia-8gb](https://github.com/soldsawn/victory-nvidia-8gb):** Base 8 GB NVIDIA Wave32 tuning (`VICTORY_TEXTURE_DROP_MIPS`, `VICTORY_WINDOW_UPLOAD`, `VICTORY_QUEUE2_REFETCH`), ambient lighting (`VICTORY_AMBIENT`), and English performance overlay.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork) (`06702fe`, `6d20eed`, `94ed363`, `b1d777a`, `0e006f0`, [PR #1315](https://github.com/fxceless/victory-fork/pull/1315), [PR #1295](https://github.com/fxceless/victory-fork/pull/1295)) & [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX) (`46e3297`, `86dc109`, `4b82f6c`, `9c23e8d`, `65f9707`, `9386b82`, `ca1d519`, `c96525d`, `ab86ef7`, `6c17ce9`):** Flight-room tuning (`VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`), masked compute upload for high-churn pages (`VICTORY_MERGE_MASK=1`), lock-free block light refresh (`VICTORY_LIGHT_REFRESH=1`), persistent uncommitted-block tracking (`VICTORY_ABSENT_ONCE=1`, `VICTORY_ABSENT_MS=told`), unlocked `VirtualProtect` re-arming (`VICTORY_GUARD_OUTSIDE=1`), unguarded view-memory `GetWriteWatch` skip, in-place 32-bit graphics heap (`VICTORY_PORT_CONSTANTS=1`), per-pipeline `TablePlan`, sharded `Made` locks + parallel `sync_heaps`/`heap_buffers` (`VICTORY_VIEW_THREADS`), `VICTORY_BIND_CACHE`, `HostList::bind_pipeline`/`fill_scratch`, wall-clock `VICTORY_TEXTURE_LOOK_MS`, and amplification-shader indirect mesh launch (`VICTORY_MESH_LAUNCH`).
- **[p1nkman-VI](https://codeberg.org/p1nkman/p1nkman-VI):** Boat-mission null-pointer `memcpy` crash guard (`null_copy` in `victory-loader`, toggleable via `VICTORY_NULL_COPY=0`).
- **marquezini:** HID gamepad support (DualSense, DualSense Edge, DualShock 4, generic HID pads with rumble).
- **codec-d/cool_fox (`773d902`, `641835a4` & fence wakes):** Batched `Host::card_before` copies with deduplicated buffer barriers, per-pipeline `TablePlan`, and `WaitOnAddress` / `WakeByAddressAll` blocking fence waits.

## What `victory-3060TI` Adds

- **100% English codebase & launchers** with clean upstream-mergeable git history.
- **GPU Mesh Launch & Safe Wave32 Shader Ops (Wave 3B/3C):**
  - Amplification-shader indirect mesh launch (`VICTORY_MESH_LAUNCH`, on by default; `0` off) places a lightweight host amplification stage (`launch.hlsl` / `launch_code.rs`) in front of mesh shaders to read, clamp, and budget-charge indirect mesh records directly on the GPU without per-draw compute clamp dispatches or pipeline flushes (`launch.rs`, `pipeline/host.rs`, `draw.rs`, `emit/shader.rs`, `emit/container.rs`).
  - Guarded 128-bit wide raw loads (`opt::WIDE = 64`, on by default with `getDimensions` bounds checks on bounded heap views), native Wave32 `ds_bpermute_b32` within 32-lane halves (after Belobug #363), and native DXIL pixel/mesh `quadReadLaneAt`/`quadOp` quad reads preserving helper lanes (after Belobug `03629f3f`) (`bitcode/opt.rs`, `lower.rs`).
- **CPU / Heap-buffer & write-watch optimizations:**
  - Parallel descriptor mirroring & lock-free `Made` hit path (`VICTORY_VIEW_THREADS=8`): shards `HostHeap::made` across 512 bucket locks + atomic keys (`Made`) and parallelizes `sync_heaps` and `heap_buffers` across `view_pool` worker threads (`view_pool.rs`, `descriptor/host.rs`, `descriptor.rs`, `draw.rs`).
  - Draw-recording deduplication & faster streamed-texture sharpening: skips redundant `OMSetRenderTargets` (`VICTORY_BIND_CACHE`), deduplicates PSO/root-signature binds (`HostList::bind_pipeline`) and unaligned root CBV scratch fills (`HostList::fill_scratch`), and caps streamed-texture retry backoff by wall-clock ms (`VICTORY_TEXTURE_LOOK_MS=500`) (`descriptor.rs`, `draw.rs`, `host.rs`).
  - In-place 32-bit graphics heap (`VICTORY_PORT_CONSTANTS=1`, Renderer Port 1): maps the title's 185 MB 32-bit graphics heap stretch directly into a host D3D12 heap via `OpenExistingHeapFromAddress1` with section slices arranged in address order (`port.rs`, `inplace.rs`, `xmem.rs`).
  - Skips doomed `GetWriteWatch` kernel calls on known view memory for both buffer and unguarded texture refreshes, plus `VICTORY_VIEW_KNOWN=1` and `VICTORY_REMAP_LEAN=1` (`watch.rs`, `xmem.rs`).
  - Masked GPU compute upload (`VICTORY_MERGE_MASK=1`) for pages with >16 changed runs under in-flight shader writes, avoiding `VICTORY_MERGE_RUNS` pipeline stalls (`card_mask.rs`, `host.rs`, `shadow.rs`).
  - Per-pipeline precomputed `TablePlan` descriptor-slot grouping (`pipeline.rs`, `draw.rs`).
  - Lock-free 64 KiB block light refresh (`VICTORY_LIGHT_REFRESH=1`), `VICTORY_ABSENT_ONCE=1` / `VICTORY_ABSENT_MS=told`, and unlocked `VirtualProtect` re-arming (`VICTORY_GUARD_OUTSIDE=1`) (`watch.rs`, `shadow.rs`, `xmem.rs`).
  - Skips table-driven buffer slots (`tabled_lately`) on the 1/16th heap-buffer refresh cadence (`draw.rs`, `descriptor/host.rs`).
  - Lock-free thread-local view lookup cache (`descriptor.rs`).
  - Batched `Host::card_before` copies, deduplicated `ResourceBarrier` transitions, and single compute root-sig/PSO bind (`host.rs`).
  - `WaitOnAddress` / `WakeByAddressAll` blocking fence waits (`fence.rs`).
- **Two ready-to-use launcher presets:**
  1. **[`scripts\play-3060ti-fps.bat`](scripts/play-3060ti-fps.bat)** *(default — playable FPS maxing)*:
     - `series-s`, `VICTORY_TEXTURE_DROP_MIPS=3`, `2048/1024 MB` card pools, `VICTORY_TEXTURE_BACK=1` (water/boat missions playable), `VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`, `VICTORY_MERGE_MASK=1`, `VICTORY_PORT_CONSTANTS=1`, `VICTORY_VIEW_THREADS=8`, `VICTORY_TEXTURE_LOOK_MS=500`, `VICTORY_VIEW_KNOWN=1`, `VICTORY_REMAP_LEAN=1`, `VICTORY_LEVELS_ON_DEMAND=0`, `VICTORY_QUEUE2_REFETCH=0`, `VICTORY_WINDOW_UPLOAD=1`, `VICTORY_AMBIENT=0.6`, quiet logging by default.
     - Optional args: `warmup` (pre-translates shaders) or `warn` (verbose logging).
  2. **[`scripts\play-3060ti-quality.bat`](scripts/play-3060ti-quality.bat)** *(quality / glitch-fix)*:
     - `series-s`, `VICTORY_TEXTURE_DROP_MIPS=2` (sharper mipmaps), `VICTORY_TEXTURE_BACK=1`, `VICTORY_LATE_WAIT_LISTS=512`, `VICTORY_MERGE_RUNS=4096`, `VICTORY_MERGE_MASK=1`, `VICTORY_PORT_CONSTANTS=1`, `VICTORY_VIEW_THREADS=8`, `VICTORY_TEXTURE_LOOK_MS=500`, `VICTORY_VIEW_KNOWN=1`, `VICTORY_REMAP_LEAN=1`, `VICTORY_AMBIENT=0.6`, `warn` logging by default.
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
