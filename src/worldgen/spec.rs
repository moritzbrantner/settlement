use crate::{GridPos, SettlementGame, TerrainType};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementMapSpec {
    pub seed: u64,
    pub width: i32,
    pub height: i32,
    pub starting_clear_radius: i32,
    pub min_buildable_tiles: usize,
    pub min_fertile_tiles: usize,
    pub min_forest_tiles: usize,
    pub min_water_tiles: usize,
    pub max_generation_attempts: u32,
}

impl Default for SettlementMapSpec {
    fn default() -> Self {
        Self {
            seed: 1,
            width: 24,
            height: 24,
            starting_clear_radius: 3,
            min_buildable_tiles: 80,
            min_fertile_tiles: 18,
            min_forest_tiles: 18,
            min_water_tiles: 12,
            max_generation_attempts: 32,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResourceRegionKind {
    Fertile,
    Forest,
    Water,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceRegion {
    pub kind: ResourceRegionKind,
    pub tiles: Vec<GridPos>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedSettlementMap {
    pub terrain: Vec<(GridPos, TerrainType)>,
    pub roads: Vec<GridPos>,
    pub recommended_spawn: GridPos,
    pub resource_regions: Vec<ResourceRegion>,
    pub playability_score: f32,
}

impl GeneratedSettlementMap {
    pub fn terrain_at(&self, position: GridPos) -> Option<TerrainType> {
        self.terrain
            .iter()
            .find_map(|(pos, terrain)| (*pos == position).then_some(*terrain))
    }

    pub fn apply_to_game(&self, game: &mut SettlementGame) {
        for (position, terrain) in &self.terrain {
            game.set_terrain(*position, *terrain);
        }
        for &road in &self.roads {
            game.place_road(road);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayabilityReport {
    pub valid: bool,
    pub playability_score: f32,
    pub buildable_tiles: usize,
    pub connected_buildable_tiles: usize,
    pub fertile_tiles: usize,
    pub forest_tiles: usize,
    pub water_tiles: usize,
    pub reachable_fertile_regions: usize,
    pub reachable_forest_regions: usize,
    pub reachable_water_access_regions: usize,
    pub failures: Vec<String>,
}
