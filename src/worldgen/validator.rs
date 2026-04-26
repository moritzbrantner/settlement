use super::{GeneratedSettlementMap, PlayabilityReport, ResourceRegionKind, SettlementMapSpec};
use crate::{GridPos, TerrainType};
use farm_nav::{NavDomain, reachable_nodes};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_playable_map(
    map: &GeneratedSettlementMap,
    spec: &SettlementMapSpec,
) -> PlayabilityReport {
    let terrain = terrain_map(map);
    let roads = map.roads.iter().copied().collect::<BTreeSet<_>>();
    let walkable = ValidationDomain {
        spec,
        terrain: &terrain,
        roads: &roads,
        mode: ValidationMode::Walkable,
    };
    let buildable = ValidationDomain {
        spec,
        terrain: &terrain,
        roads: &roads,
        mode: ValidationMode::BuildableOnly,
    };

    let spawn = map.recommended_spawn;
    let reachable_walkable = if terrain
        .get(&spawn)
        .copied()
        .unwrap_or(TerrainType::Blocked)
        .is_walkable()
    {
        reachable_nodes(&walkable, spawn, None, ())
    } else {
        BTreeMap::new()
    };
    let reachable_buildable = if terrain
        .get(&spawn)
        .copied()
        .unwrap_or(TerrainType::Blocked)
        .is_buildable()
    {
        reachable_nodes(&buildable, spawn, None, ())
    } else {
        BTreeMap::new()
    };

    let buildable_tiles = terrain
        .values()
        .filter(|terrain| terrain.is_buildable())
        .count();
    let fertile_tiles = terrain
        .values()
        .filter(|terrain| **terrain == TerrainType::Fertile)
        .count();
    let forest_tiles = terrain
        .values()
        .filter(|terrain| **terrain == TerrainType::Forest)
        .count();
    let water_tiles = terrain
        .values()
        .filter(|terrain| **terrain == TerrainType::Water)
        .count();

    let reachable_fertile_regions = map
        .resource_regions
        .iter()
        .filter(|region| region.kind == ResourceRegionKind::Fertile)
        .filter(|region| {
            region
                .tiles
                .iter()
                .any(|tile| reachable_walkable.contains_key(tile))
        })
        .count();
    let reachable_forest_regions = map
        .resource_regions
        .iter()
        .filter(|region| region.kind == ResourceRegionKind::Forest)
        .filter(|region| {
            region
                .tiles
                .iter()
                .any(|tile| reachable_walkable.contains_key(tile))
        })
        .count();
    let reachable_water_access_regions = map
        .resource_regions
        .iter()
        .filter(|region| region.kind == ResourceRegionKind::Water)
        .filter(|region| {
            region.tiles.iter().any(|tile| {
                cardinal_neighbors(spec, *tile)
                    .into_iter()
                    .any(|neighbor| reachable_walkable.contains_key(&neighbor))
            })
        })
        .count();

    let mut failures = Vec::new();
    if reachable_buildable.len() < spec.min_buildable_tiles {
        failures.push(format!(
            "connected buildable core too small: {} < {}",
            reachable_buildable.len(),
            spec.min_buildable_tiles
        ));
    }
    if fertile_tiles < spec.min_fertile_tiles {
        failures.push(format!(
            "fertile tiles below minimum: {fertile_tiles} < {}",
            spec.min_fertile_tiles
        ));
    }
    if forest_tiles < spec.min_forest_tiles {
        failures.push(format!(
            "forest tiles below minimum: {forest_tiles} < {}",
            spec.min_forest_tiles
        ));
    }
    if water_tiles < spec.min_water_tiles {
        failures.push(format!(
            "water tiles below minimum: {water_tiles} < {}",
            spec.min_water_tiles
        ));
    }
    if reachable_fertile_regions == 0 {
        failures.push("no reachable fertile region".into());
    }
    if reachable_forest_regions == 0 {
        failures.push("no reachable forest region".into());
    }
    if spec.min_water_tiles > 0 && reachable_water_access_regions == 0 {
        failures.push("no reachable water access region".into());
    }

    let score = score_ratio(reachable_buildable.len(), spec.min_buildable_tiles)
        + score_ratio(fertile_tiles, spec.min_fertile_tiles)
        + score_ratio(forest_tiles, spec.min_forest_tiles)
        + score_ratio(water_tiles, spec.min_water_tiles.max(1))
        + (reachable_fertile_regions > 0) as u8 as f32
        + (reachable_forest_regions > 0) as u8 as f32
        + ((spec.min_water_tiles == 0 || reachable_water_access_regions > 0) as u8 as f32);

    PlayabilityReport {
        valid: failures.is_empty(),
        playability_score: score,
        buildable_tiles,
        connected_buildable_tiles: reachable_buildable.len(),
        fertile_tiles,
        forest_tiles,
        water_tiles,
        reachable_fertile_regions,
        reachable_forest_regions,
        reachable_water_access_regions,
        failures,
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ValidationMode {
    Walkable,
    BuildableOnly,
}

struct ValidationDomain<'a> {
    spec: &'a SettlementMapSpec,
    terrain: &'a BTreeMap<GridPos, TerrainType>,
    roads: &'a BTreeSet<GridPos>,
    mode: ValidationMode,
}

impl NavDomain for ValidationDomain<'_> {
    type Node = GridPos;
    type Profile = ();

    fn neighbors(&self, node: Self::Node, _: Self::Profile, out: &mut Vec<(Self::Node, f32)>) {
        out.clear();
        for neighbor in cardinal_neighbors(self.spec, node) {
            let terrain = self
                .terrain
                .get(&neighbor)
                .copied()
                .unwrap_or(TerrainType::Blocked);
            let edge_cost = match self.mode {
                ValidationMode::Walkable => self
                    .roads
                    .contains(&neighbor)
                    .then_some(0.5_f32)
                    .or_else(|| terrain.movement_cost().map(|cost| cost as f32)),
                ValidationMode::BuildableOnly => terrain.is_buildable().then_some(1.0),
            };
            let Some(edge_cost) = edge_cost else {
                continue;
            };
            out.push((neighbor, edge_cost));
        }
    }

    fn heuristic(&self, from: Self::Node, goal: Self::Node, _: Self::Profile) -> f32 {
        from.manhattan_distance(goal) as f32
    }

    fn version(&self) -> u64 {
        0
    }
}

fn terrain_map(map: &GeneratedSettlementMap) -> BTreeMap<GridPos, TerrainType> {
    map.terrain.iter().copied().collect()
}

fn cardinal_neighbors(spec: &SettlementMapSpec, position: GridPos) -> Vec<GridPos> {
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| GridPos::new(position.x + dx, position.y + dy))
        .filter(|position| {
            (0..spec.width).contains(&position.x) && (0..spec.height).contains(&position.y)
        })
        .collect()
}

fn score_ratio(actual: usize, minimum: usize) -> f32 {
    if minimum == 0 {
        return 1.0;
    }
    (actual as f32 / minimum as f32).min(1.0)
}
