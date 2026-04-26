# Settlement

## Status

- Prototype.
- Intentionally not a full repo-integrated vertical slice in the current repo state.
- Present on disk as the `settlement_game` crate, but excluded from the root workspace and root CI surface.

## High-Level Pitch

Settlement is a deterministic day-and-night prototype about optimizing a compact farming settlement during the day and surviving organized raids at night. The current implementation is primarily a simulation and balancing surface rather than a player-facing product slice.

## Player Fantasy

The player is tuning a vulnerable frontier settlement. The intended fantasy is to place production and storage intelligently, shorten logistics routes, prepare layered defenses, and absorb or repel raids without letting raiders strip the economy bare.

## Current Implemented Experience

The strongest evidence for the current game comes from the `SettlementGame` simulation API, the prototype tests in `games/settlement/tests/prototype.rs`, and the browser prototype in `games/settlement/visualization`.

The prototype already supports a grid map, terrain costs, roads, production buildings, storage buildings, worker logistics, blockers, defenses, raid lanes, multi-wave raids, looting, retreat, ruin creation, resource recovery, and debug metrics. The repo README also states that this crate is intentionally excluded from workspace-level commands and CI coverage in the current state.

This should be read as a system prototype. It is not yet a polished, repo-integrated vertical slice.

## Core Gameplay Loop

- Place production and storage buildings to create efficient daytime logistics.
- Shorten worker delivery routes with map layout and roads.
- Decide where obstacles and defenses should shape nighttime raid paths.
- Let the day advance until the raid trigger or trigger raids early for testing.
- Survive multi-wave attacks that target storage value and accessible routes.
- Recover from damage, stolen resources, destroyed blockers, and ruins.
- Iterate on layout and balance to improve next-day survival and throughput.

## Core Systems

- Grid map: bounded tile map with cardinal movement and explicit lanes.
- Terrain and roads: movement cost changes directly affect worker and raider routing.
- Production buildings: generate buffered output over time.
- Storage buildings: constrain accepted resources and total capacity.
- Workers: implicitly created around production flow and responsible for delivery behavior.
- Obstacles and defenses: blockers can absorb or redirect raids, while defenses attack raiders over time.
- Raid lanes and waves: raids spawn through named lanes with delayed multi-wave definitions.
- Raider states: spawned, moving, destroying blockers, looting, retreating, escaped, and dead.
- Raid outcomes: stolen resources, recovery pool, destroyed buildings, blockers destroyed, killed raiders, escaped raiders, and damage dealt.
- Debug metrics: throughput, path cost, stolen and recovered resources, raid timing, and destruction summaries are explicit outputs.

## World, Content, and Entities

The prototype models a compact settlement on a tile grid. Buildings include production, storage, defense, obstacles, and ruins. Workers and raiders move over the same map but with different routing goals and constraints.

Content breadth is intentionally thin. The value of the prototype is in the interaction between route cost, storage placement, raid targeting, defense timing, and recovery outcomes.

## Progression, Objectives, and Failure Pressure

Settlement currently expresses pressure more through simulation outcomes than through formal player-facing objectives.

- Daytime pressure comes from path inefficiency, distant storage, blocked routes, and production output lost to poor logistics.
- Nighttime pressure comes from raid timing, wave composition, blocker destruction, storage looting, and raider escapes.
- Recovery pressure comes from partial resource recovery after kills, ruined blockers or buildings, and the cumulative cost of surviving with a weak layout.

Unlike Zoo and Raid Defense, the prototype does not yet expose a polished objective, alert, or win-state layer. The balancing signals live mostly in tests, raid results, and debug metrics.

## Current Interfaces and Surfaces

- Rust prototype API: `SettlementGame` exposes simulation construction, economy ticking, raid ticking, map edits, and raid configuration.
- Deterministic simulation harness: tests exercise the core loop directly through code and assert on measurable outcomes.
- Repo-local crate surface: the crate can be worked on independently, but it is not wired into the root workspace members.
- Browser prototype: `games/settlement/visualization` provides a local visualization surface for layout and pacing iteration.
- No integrated server surface is present in this repo state.
- The browser prototype remains lightweight and is not yet a polished production-facing UI surface.

## Constraints and Known Gaps

- Prototype status should not be confused with production readiness.
- Campaign framing and long-term progression are missing.
- A player-facing UI and onboarding layer are missing.
- Content breadth is narrow and tuned for systems exploration.
- Workspace-level integration and CI coverage are intentionally absent from the root repo configuration.

## Near-Term Roadmap

The next coherent milestone is to tighten the prototype into a clearer optimize-by-day, survive-by-night loop.

- Improve balancing so logistics gains and defensive preparation produce clearer player-visible tradeoffs.
- Add clearer scenario framing around starting layouts, raid patterns, or authored test situations.
- Make the prototype easier to read as a repeatable game loop rather than only as a systems sandbox.
- Preserve the bounded prototype scope instead of expanding immediately into a full survival city-builder or campaign stack.

## Source Anchors

- [../../README.md](../../README.md)
- [../../Cargo.toml](../../Cargo.toml)
- [src/lib.rs](src/lib.rs)
- [tests/prototype.rs](tests/prototype.rs)
- [visualization/package.json](visualization/package.json)
