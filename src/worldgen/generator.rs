use super::{
    GeneratedSettlementMap, ResourceRegion, ResourceRegionKind, SettlementMapSpec,
    validate_playable_map,
};
use crate::{GridPos, TerrainType};
use std::collections::BTreeSet;

const RETRY_STRIDE: u64 = 0x9e37_79b9_7f4a_7c15;

pub fn generate_playable_map(spec: &SettlementMapSpec) -> GeneratedSettlementMap {
    let attempts = spec.max_generation_attempts.max(1);
    let mut best_candidate: Option<GeneratedSettlementMap> = None;
    let mut best_score = f32::NEG_INFINITY;

    for attempt in 0..attempts {
        let seed = spec
            .seed
            .wrapping_add(RETRY_STRIDE.wrapping_mul(attempt as u64));
        let mut candidate = generate_candidate(spec, seed);
        let report = validate_playable_map(&candidate, spec);
        candidate.playability_score = report.playability_score;
        if report.valid {
            return candidate;
        }
        if candidate.playability_score > best_score {
            best_score = candidate.playability_score;
            best_candidate = Some(candidate);
        }
    }

    best_candidate.unwrap_or_else(|| generate_candidate(spec, spec.seed))
}

fn generate_candidate(spec: &SettlementMapSpec, seed: u64) -> GeneratedSettlementMap {
    assert!(
        spec.width > 0 && spec.height > 0,
        "map dimensions must be positive"
    );
    let mut rng = SeededRng::new(seed);
    let mut terrain = vec![TerrainType::Ground; (spec.width * spec.height) as usize];
    let spawn = GridPos::new(spec.width / 2, spec.height / 2);
    let protected = protected_positions(spec, spawn);

    let water_blobs = ((spec.min_water_tiles.max(1) + 11) / 12).clamp(1, 4);
    let forest_blobs = ((spec.min_forest_tiles.max(1) + 11) / 12).clamp(1, 5);
    let fertile_blobs = ((spec.min_fertile_tiles.max(1) + 11) / 12).clamp(1, 5);

    for _ in 0..water_blobs {
        if let Some(center) =
            random_center(spec, &protected, &mut rng, spec.starting_clear_radius + 3)
        {
            paint_blob(
                spec,
                &mut terrain,
                center,
                2 + rng.next_bounded_i32(3),
                TerrainType::Water,
                &protected,
                &mut rng,
            );
        }
    }
    ensure_minimum(
        spec,
        &mut terrain,
        TerrainType::Water,
        spec.min_water_tiles,
        &protected,
        &mut rng,
    );

    for _ in 0..forest_blobs {
        if let Some(center) =
            random_center(spec, &protected, &mut rng, spec.starting_clear_radius + 1)
        {
            paint_blob(
                spec,
                &mut terrain,
                center,
                2 + rng.next_bounded_i32(3),
                TerrainType::Forest,
                &protected,
                &mut rng,
            );
        }
    }
    ensure_minimum(
        spec,
        &mut terrain,
        TerrainType::Forest,
        spec.min_forest_tiles,
        &protected,
        &mut rng,
    );

    for _ in 0..fertile_blobs {
        if let Some(center) =
            random_center(spec, &protected, &mut rng, spec.starting_clear_radius + 1)
        {
            paint_blob(
                spec,
                &mut terrain,
                center,
                2 + rng.next_bounded_i32(2),
                TerrainType::Fertile,
                &protected,
                &mut rng,
            );
        }
    }
    ensure_minimum(
        spec,
        &mut terrain,
        TerrainType::Fertile,
        spec.min_fertile_tiles,
        &protected,
        &mut rng,
    );

    if current_count(&terrain, TerrainType::Ground) < spec.min_buildable_tiles {
        fill_buildable(
            spec,
            &mut terrain,
            spec.min_buildable_tiles,
            &protected,
            &mut rng,
        );
    }

    // Keep the starting core clean and connected.
    for position in &protected {
        set_terrain(spec, &mut terrain, *position, TerrainType::Ground);
    }

    let roads = spawn_roads(spec, spawn);
    let resource_regions = collect_resource_regions(spec, &terrain);
    GeneratedSettlementMap {
        terrain: terrain
            .into_iter()
            .enumerate()
            .map(|(index, terrain)| (index_to_pos(spec, index), terrain))
            .collect(),
        roads,
        recommended_spawn: spawn,
        resource_regions,
        playability_score: 0.0,
    }
}

fn collect_resource_regions(
    spec: &SettlementMapSpec,
    terrain: &[TerrainType],
) -> Vec<ResourceRegion> {
    let mut visited = BTreeSet::new();
    let mut regions = Vec::new();

    for y in 0..spec.height {
        for x in 0..spec.width {
            let position = GridPos::new(x, y);
            if visited.contains(&position) {
                continue;
            }
            let kind = match terrain_at(spec, terrain, position) {
                TerrainType::Fertile => Some(ResourceRegionKind::Fertile),
                TerrainType::Forest => Some(ResourceRegionKind::Forest),
                TerrainType::Water => Some(ResourceRegionKind::Water),
                _ => None,
            };
            let Some(region_kind) = kind else {
                continue;
            };
            let mut stack = vec![position];
            let mut tiles = Vec::new();
            visited.insert(position);

            while let Some(current) = stack.pop() {
                tiles.push(current);
                for neighbor in neighbors(spec, current) {
                    if visited.contains(&neighbor) {
                        continue;
                    }
                    let neighbor_kind = match terrain_at(spec, terrain, neighbor) {
                        TerrainType::Fertile => Some(ResourceRegionKind::Fertile),
                        TerrainType::Forest => Some(ResourceRegionKind::Forest),
                        TerrainType::Water => Some(ResourceRegionKind::Water),
                        _ => None,
                    };
                    if neighbor_kind == Some(region_kind) {
                        visited.insert(neighbor);
                        stack.push(neighbor);
                    }
                }
            }

            regions.push(ResourceRegion {
                kind: region_kind,
                tiles,
            });
        }
    }

    regions.sort_by_key(|region| (region.tiles.len(), region.tiles[0]));
    regions
}

fn protected_positions(spec: &SettlementMapSpec, spawn: GridPos) -> BTreeSet<GridPos> {
    let mut protected = BTreeSet::new();
    for y in 0..spec.height {
        for x in 0..spec.width {
            let position = GridPos::new(x, y);
            if spawn.manhattan_distance(position) <= spec.starting_clear_radius {
                protected.insert(position);
            }
        }
    }
    protected
}

fn random_center(
    spec: &SettlementMapSpec,
    protected: &BTreeSet<GridPos>,
    rng: &mut SeededRng,
    min_distance_from_spawn: i32,
) -> Option<GridPos> {
    let spawn = GridPos::new(spec.width / 2, spec.height / 2);
    for _ in 0..256 {
        let position = GridPos::new(
            rng.next_bounded_i32(spec.width),
            rng.next_bounded_i32(spec.height),
        );
        if protected.contains(&position)
            || spawn.manhattan_distance(position) < min_distance_from_spawn
        {
            continue;
        }
        return Some(position);
    }
    None
}

fn ensure_minimum(
    spec: &SettlementMapSpec,
    terrain: &mut [TerrainType],
    target: TerrainType,
    minimum: usize,
    protected: &BTreeSet<GridPos>,
    rng: &mut SeededRng,
) {
    while current_count(terrain, target) < minimum {
        let position = GridPos::new(
            rng.next_bounded_i32(spec.width),
            rng.next_bounded_i32(spec.height),
        );
        if protected.contains(&position) {
            continue;
        }
        set_terrain(spec, terrain, position, target);
    }
}

fn fill_buildable(
    spec: &SettlementMapSpec,
    terrain: &mut [TerrainType],
    minimum_buildable: usize,
    protected: &BTreeSet<GridPos>,
    rng: &mut SeededRng,
) {
    while terrain.iter().filter(|tile| tile.is_buildable()).count() < minimum_buildable {
        let position = GridPos::new(
            rng.next_bounded_i32(spec.width),
            rng.next_bounded_i32(spec.height),
        );
        if protected.contains(&position) {
            continue;
        }
        set_terrain(spec, terrain, position, TerrainType::Ground);
    }
}

fn paint_blob(
    spec: &SettlementMapSpec,
    terrain: &mut [TerrainType],
    center: GridPos,
    radius: i32,
    target: TerrainType,
    protected: &BTreeSet<GridPos>,
    rng: &mut SeededRng,
) {
    for y in (center.y - radius)..=(center.y + radius) {
        for x in (center.x - radius)..=(center.x + radius) {
            let position = GridPos::new(x, y);
            if !in_bounds(spec, position) || protected.contains(&position) {
                continue;
            }
            if center.manhattan_distance(position) > radius {
                continue;
            }
            if center.manhattan_distance(position) == radius && !rng.chance(2, 3) {
                continue;
            }
            set_terrain(spec, terrain, position, target);
        }
    }
}

fn spawn_roads(spec: &SettlementMapSpec, spawn: GridPos) -> Vec<GridPos> {
    let arm = spec.starting_clear_radius.min(2).max(1);
    let mut roads = vec![spawn];
    for offset in 1..=arm {
        for position in [
            GridPos::new(spawn.x + offset, spawn.y),
            GridPos::new(spawn.x - offset, spawn.y),
            GridPos::new(spawn.x, spawn.y + offset),
            GridPos::new(spawn.x, spawn.y - offset),
        ] {
            if in_bounds(spec, position) {
                roads.push(position);
            }
        }
    }
    roads.sort();
    roads.dedup();
    roads
}

fn terrain_at(spec: &SettlementMapSpec, terrain: &[TerrainType], position: GridPos) -> TerrainType {
    terrain[pos_to_index(spec, position)]
}

fn set_terrain(
    spec: &SettlementMapSpec,
    terrain: &mut [TerrainType],
    position: GridPos,
    value: TerrainType,
) {
    let index = pos_to_index(spec, position);
    terrain[index] = value;
}

fn current_count(terrain: &[TerrainType], target: TerrainType) -> usize {
    terrain.iter().filter(|terrain| **terrain == target).count()
}

fn pos_to_index(spec: &SettlementMapSpec, position: GridPos) -> usize {
    (position.y * spec.width + position.x) as usize
}

fn index_to_pos(spec: &SettlementMapSpec, index: usize) -> GridPos {
    let x = index as i32 % spec.width;
    let y = index as i32 / spec.width;
    GridPos::new(x, y)
}

fn neighbors(spec: &SettlementMapSpec, position: GridPos) -> Vec<GridPos> {
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| GridPos::new(position.x + dx, position.y + dy))
        .filter(|position| in_bounds(spec, *position))
        .collect()
}

fn in_bounds(spec: &SettlementMapSpec, position: GridPos) -> bool {
    (0..spec.width).contains(&position.x) && (0..spec.height).contains(&position.y)
}
#[derive(Clone, Debug)]
struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        let mixed = seed ^ 0xa076_1d64_78bd_642f;
        Self {
            state: if mixed == 0 { 1 } else { mixed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn next_bounded_i32(&mut self, upper_exclusive: i32) -> i32 {
        if upper_exclusive <= 1 {
            return 0;
        }
        (self.next_u64() % upper_exclusive as u64) as i32
    }

    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        if denominator == 0 || numerator >= denominator {
            return true;
        }
        self.next_u64() % denominator < numerator
    }
}
