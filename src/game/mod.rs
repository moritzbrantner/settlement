mod aliases;
mod defense;
mod economy;
mod metrics;
mod navigation;
mod raiders;

use self::defense::DefenseProjectile;
use crate::raid::default_raid_definition;
use crate::{
    Building, BuildingId, BuildingKind, DebugMetrics, DefenseBuilding, GameMap, GridPos,
    LaneDefinition, Obstacle, PathResult, ProductionBuilding, RaidDefinition, RaidResult, Raider,
    RaiderId, Road, StorageBuilding, TerrainType, Worker, WorkerId, WorkerState,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct SimulationConfig {
    pub day_length: f64,
    pub stored_value_weight: f64,
    pub path_cost_weight: f64,
    pub simulation_step: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            day_length: 24.0 * 60.0 * 60.0,
            stored_value_weight: 1.0,
            path_cost_weight: 1.0,
            simulation_step: 0.25,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettlementGame {
    pub map: GameMap,
    pub buildings: BTreeMap<BuildingId, Building>,
    pub workers: BTreeMap<WorkerId, Worker>,
    pub raiders: BTreeMap<RaiderId, Raider>,
    pub config: SimulationConfig,
    pub default_raid_definition: RaidDefinition,
    pub recovery_pool: BTreeMap<String, f64>,
    next_building_id: BuildingId,
    next_worker_id: WorkerId,
    next_raider_id: RaiderId,
    current_day_elapsed: f64,
    active_raid: Option<ActiveRaid>,
    debug: DebugMetrics,
    worker_routes: BTreeMap<WorkerId, RouteProgress>,
    raider_routes: BTreeMap<RaiderId, RouteProgress>,
    defense_projectiles: Vec<DefenseProjectile>,
    last_raid_result: Option<RaidResult>,
    paths_dirty: bool,
    nav_version: u64,
    delivery_path_cost_sum: f64,
    delivery_path_cost_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveRaid {
    definition: RaidDefinition,
    elapsed: f64,
    spawned_waves: BTreeSet<usize>,
    result: RaidResult,
}

#[derive(Clone, Debug, PartialEq)]
struct RouteProgress {
    path: Vec<GridPos>,
    next_index: usize,
    progress_on_edge: f64,
    total_cost: f64,
}

impl RouteProgress {
    fn from_path(path: &PathResult) -> Option<Self> {
        path.found.then(|| Self {
            path: path.path.clone(),
            next_index: 0,
            progress_on_edge: 0.0,
            total_cost: path.total_movement_cost,
        })
    }

    fn is_finished(&self) -> bool {
        self.next_index + 1 >= self.path.len()
    }
}

impl SettlementGame {
    pub fn new(width: i32, height: i32) -> Self {
        let mut map = GameMap::new(width, height);
        map.add_lane(LaneDefinition::new(
            "north",
            GridPos::new(width / 2, 0),
            GridPos::new(width / 2, 0),
        ));
        Self {
            map,
            buildings: BTreeMap::new(),
            workers: BTreeMap::new(),
            raiders: BTreeMap::new(),
            config: SimulationConfig::default(),
            default_raid_definition: default_raid_definition(),
            recovery_pool: BTreeMap::new(),
            next_building_id: 1,
            next_worker_id: 1,
            next_raider_id: 1,
            current_day_elapsed: 0.0,
            active_raid: None,
            debug: DebugMetrics::default(),
            worker_routes: BTreeMap::new(),
            raider_routes: BTreeMap::new(),
            defense_projectiles: Vec::new(),
            last_raid_result: None,
            paths_dirty: false,
            nav_version: 0,
            delivery_path_cost_sum: 0.0,
            delivery_path_cost_count: 0,
        }
    }

    pub fn with_config(mut self, config: SimulationConfig) -> Self {
        self.config = config;
        self
    }

    pub fn set_default_raid_definition(&mut self, raid_definition: RaidDefinition) {
        self.default_raid_definition = raid_definition;
    }

    pub fn add_lane(&mut self, lane: LaneDefinition) {
        self.map.add_lane(lane);
    }

    pub fn set_terrain(&mut self, pos: GridPos, terrain: TerrainType) {
        self.map.set_terrain(pos, terrain);
        self.recalculate_paths_after_map_change();
    }

    pub fn place_road(&mut self, pos: GridPos) {
        self.map.place_road(pos, Road::default());
        self.recalculate_paths_after_map_change();
    }

    pub fn add_building(&mut self, mut building: Building) -> BuildingId {
        assert!(
            self.map.in_bounds(building.position),
            "building out of bounds"
        );
        assert!(
            self.map.tile(building.position).building.is_none(),
            "tile already occupied"
        );
        let building_id = self.next_building_id;
        self.next_building_id += 1;
        building.id = building_id;
        self.map.tile_mut(building.position).building = Some(building_id);
        let create_worker = matches!(building.kind, BuildingKind::Production(_));
        let worker_position = building.position;
        self.buildings.insert(building_id, building);
        if create_worker {
            let worker_id = self.next_worker_id;
            self.next_worker_id += 1;
            self.workers.insert(
                worker_id,
                Worker {
                    id: worker_id,
                    home_production: building_id,
                    position: worker_position,
                    carry_capacity: 10.0,
                    carrying: None,
                    state: WorkerState::Idle,
                },
            );
        }
        self.recalculate_paths_after_map_change();
        building_id
    }

    pub fn add_production_building(
        &mut self,
        position: GridPos,
        resource_type: impl Into<String>,
        generation_rate_per_second: f64,
        output_buffer_capacity: f64,
    ) -> BuildingId {
        self.add_building(Building::production(
            position,
            ProductionBuilding::new(
                resource_type,
                generation_rate_per_second,
                output_buffer_capacity,
            ),
        ))
    }

    pub fn add_storage_building(&mut self, position: GridPos, capacity: f64) -> BuildingId {
        self.add_building(Building::storage(position, StorageBuilding::new(capacity)))
    }

    pub fn add_defense_building(
        &mut self,
        position: GridPos,
        damage: f64,
        range: i32,
        attack_cooldown: f64,
    ) -> BuildingId {
        self.add_building(Building::defense(
            position,
            DefenseBuilding::new(damage, range, attack_cooldown),
        ))
    }

    pub fn add_obstacle(&mut self, position: GridPos, hit_points: f64) -> BuildingId {
        self.add_building(Building::obstacle(position, Obstacle::new(hit_points)))
    }

    pub fn add_storage_resource(&mut self, storage_id: BuildingId, resource: &str, amount: f64) {
        if let Some(storage) = self
            .buildings
            .get_mut(&storage_id)
            .and_then(Building::storage_data_mut)
        {
            let accepted_amount = amount.min(storage.free_capacity());
            if accepted_amount > 0.0 {
                *storage.stored.entry(resource.into()).or_default() += accepted_amount;
            }
        }
    }
}
