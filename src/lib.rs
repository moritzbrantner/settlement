use farm_nav::{NavDomain, PathRequest, astar};
use std::collections::{BTreeMap, BTreeSet};

mod worldgen;

pub use worldgen::{
    GeneratedSettlementMap, PlayabilityReport, ResourceRegion, ResourceRegionKind,
    SettlementMapSpec, generate_playable_map, validate_playable_map,
};

pub type TileMap = GameMap;
pub type Wall = Obstacle;
pub type ObstacleBuilding = Obstacle;

const EPSILON: f64 = 0.000_001;
const MIN_MOVEMENT_COST: f64 = 0.5;
const BLOCKER_PENALTY_COST: f64 = 25.0;
const ARROW_PROJECTILE_SPEED: f64 = 6.0;
const ROCK_PROJECTILE_SPEED: f64 = 2.5;
const ROCK_DAMAGE_MULTIPLIER: f64 = 0.5;
const ROCK_COOLDOWN_MULTIPLIER: f64 = 2.0;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn manhattan_distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceStack {
    pub resource: String,
    pub amount: f64,
}

impl ResourceStack {
    pub fn new(resource: impl Into<String>, amount: f64) -> Self {
        Self {
            resource: resource.into(),
            amount,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TerrainType {
    Ground,
    Rough,
    Fertile,
    Forest,
    Water,
    Blocked,
}

impl TerrainType {
    fn movement_cost(self) -> Option<f64> {
        match self {
            Self::Ground => Some(1.0),
            Self::Rough => Some(2.0),
            Self::Fertile => Some(1.0),
            Self::Forest => Some(2.5),
            Self::Water => None,
            Self::Blocked => None,
        }
    }

    pub fn is_walkable(self) -> bool {
        self.movement_cost().is_some()
    }

    pub fn is_buildable(self) -> bool {
        matches!(self, Self::Ground | Self::Rough | Self::Fertile)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Road {
    pub movement_cost: f64,
}

impl Default for Road {
    fn default() -> Self {
        Self { movement_cost: 0.5 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub terrain: TerrainType,
    pub road: Option<Road>,
    pub building: Option<BuildingId>,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            terrain: TerrainType::Ground,
            road: None,
            building: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameMap {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    pub lanes: BTreeMap<String, LaneDefinition>,
}

impl GameMap {
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0, "map dimensions must be positive");
        Self {
            width,
            height,
            tiles: vec![Tile::default(); (width * height) as usize],
            lanes: BTreeMap::new(),
        }
    }

    pub fn in_bounds(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    pub fn neighbors(&self, pos: GridPos) -> [Option<GridPos>; 4] {
        [
            self.in_bounds(GridPos::new(pos.x + 1, pos.y))
                .then_some(GridPos::new(pos.x + 1, pos.y)),
            self.in_bounds(GridPos::new(pos.x - 1, pos.y))
                .then_some(GridPos::new(pos.x - 1, pos.y)),
            self.in_bounds(GridPos::new(pos.x, pos.y + 1))
                .then_some(GridPos::new(pos.x, pos.y + 1)),
            self.in_bounds(GridPos::new(pos.x, pos.y - 1))
                .then_some(GridPos::new(pos.x, pos.y - 1)),
        ]
    }

    pub fn tile(&self, pos: GridPos) -> &Tile {
        &self.tiles[self.index(pos)]
    }

    pub fn tile_mut(&mut self, pos: GridPos) -> &mut Tile {
        let index = self.index(pos);
        &mut self.tiles[index]
    }

    pub fn set_terrain(&mut self, pos: GridPos, terrain: TerrainType) {
        self.tile_mut(pos).terrain = terrain;
    }

    pub fn place_road(&mut self, pos: GridPos, road: Road) {
        self.tile_mut(pos).road = Some(road);
    }

    pub fn remove_road(&mut self, pos: GridPos) {
        self.tile_mut(pos).road = None;
    }

    pub fn add_lane(&mut self, lane: LaneDefinition) {
        self.lanes.insert(lane.id.clone(), lane);
    }

    fn index(&self, pos: GridPos) -> usize {
        assert!(self.in_bounds(pos), "position out of bounds");
        (pos.y * self.width + pos.x) as usize
    }
}

pub type BuildingId = u32;
pub type WorkerId = u32;
pub type RaiderId = u32;

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionBuilding {
    pub resource_type: String,
    pub generation_rate_per_second: f64,
    pub output_buffer_capacity: f64,
    pub output_buffer: ResourceStack,
}

impl ProductionBuilding {
    pub fn new(
        resource_type: impl Into<String>,
        generation_rate_per_second: f64,
        output_buffer_capacity: f64,
    ) -> Self {
        let resource_type = resource_type.into();
        Self {
            resource_type: resource_type.clone(),
            generation_rate_per_second,
            output_buffer_capacity,
            output_buffer: ResourceStack::new(resource_type, 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StorageBuilding {
    pub accepted_resources: Option<BTreeSet<String>>,
    pub capacity: f64,
    pub stored: BTreeMap<String, f64>,
}

impl StorageBuilding {
    pub fn new(capacity: f64) -> Self {
        Self {
            accepted_resources: None,
            capacity,
            stored: BTreeMap::new(),
        }
    }

    pub fn accepts(&self, resource: &str) -> bool {
        self.accepted_resources
            .as_ref()
            .is_none_or(|accepted| accepted.contains(resource))
    }

    pub fn total_stored(&self) -> f64 {
        self.stored.values().copied().sum()
    }

    pub fn free_capacity(&self) -> f64 {
        (self.capacity - self.total_stored()).max(0.0)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DefenseTargetPriority {
    FirstInRange,
    LowestHealth,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DefenseProjectileKind {
    Arrow,
    Rock,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DefenseWeapon {
    pub projectile_kind: DefenseProjectileKind,
    pub damage: f64,
    pub range: i32,
    pub attack_cooldown: f64,
    pub cooldown_remaining: f64,
    pub projectile_speed: f64,
    pub tracking: bool,
}

impl DefenseWeapon {
    pub fn arrow(damage: f64, range: i32, attack_cooldown: f64) -> Self {
        Self {
            projectile_kind: DefenseProjectileKind::Arrow,
            damage,
            range,
            attack_cooldown,
            cooldown_remaining: 0.0,
            projectile_speed: ARROW_PROJECTILE_SPEED,
            tracking: true,
        }
    }

    pub fn rock(damage: f64, range: i32, attack_cooldown: f64) -> Self {
        Self {
            projectile_kind: DefenseProjectileKind::Rock,
            damage,
            range,
            attack_cooldown,
            cooldown_remaining: 0.0,
            projectile_speed: ROCK_PROJECTILE_SPEED,
            tracking: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefenseBuilding {
    pub weapons: Vec<DefenseWeapon>,
    pub targeting_priority: DefenseTargetPriority,
}

impl DefenseBuilding {
    pub fn new(damage: f64, range: i32, attack_cooldown: f64) -> Self {
        let rock_range = if range > 1 { range - 1 } else { 1 };
        Self {
            weapons: vec![
                DefenseWeapon::arrow(damage, range, attack_cooldown),
                DefenseWeapon::rock(
                    damage * ROCK_DAMAGE_MULTIPLIER,
                    rock_range,
                    attack_cooldown * ROCK_COOLDOWN_MULTIPLIER,
                ),
            ],
            targeting_priority: DefenseTargetPriority::FirstInRange,
        }
    }

    pub fn with_weapons(mut self, weapons: Vec<DefenseWeapon>) -> Self {
        self.weapons = weapons;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle {
    pub hit_points: f64,
    pub max_hit_points: f64,
}

impl Obstacle {
    pub fn new(hit_points: f64) -> Self {
        Self {
            hit_points,
            max_hit_points: hit_points,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuinMarker {
    pub original_kind: String,
    pub rebuild_discount: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BuildingKind {
    Production(ProductionBuilding),
    Storage(StorageBuilding),
    Defense(DefenseBuilding),
    Obstacle(Obstacle),
    Ruin(RuinMarker),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    pub id: BuildingId,
    pub position: GridPos,
    pub hit_points: f64,
    pub max_hit_points: f64,
    pub walkable: bool,
    pub targetable: bool,
    pub kind: BuildingKind,
}

impl Building {
    pub fn production(position: GridPos, production: ProductionBuilding) -> Self {
        Self {
            id: 0,
            position,
            hit_points: 25.0,
            max_hit_points: 25.0,
            walkable: true,
            targetable: true,
            kind: BuildingKind::Production(production),
        }
    }

    pub fn storage(position: GridPos, storage: StorageBuilding) -> Self {
        Self {
            id: 0,
            position,
            hit_points: 40.0,
            max_hit_points: 40.0,
            walkable: true,
            targetable: true,
            kind: BuildingKind::Storage(storage),
        }
    }

    pub fn defense(position: GridPos, defense: DefenseBuilding) -> Self {
        Self {
            id: 0,
            position,
            hit_points: 30.0,
            max_hit_points: 30.0,
            walkable: true,
            targetable: true,
            kind: BuildingKind::Defense(defense),
        }
    }

    pub fn obstacle(position: GridPos, obstacle: Obstacle) -> Self {
        let hit_points = obstacle.hit_points;
        Self {
            id: 0,
            position,
            hit_points,
            max_hit_points: obstacle.max_hit_points,
            walkable: false,
            targetable: true,
            kind: BuildingKind::Obstacle(obstacle),
        }
    }

    pub fn ruin(position: GridPos, original_kind: impl Into<String>) -> Self {
        Self {
            id: 0,
            position,
            hit_points: 0.0,
            max_hit_points: 0.0,
            walkable: true,
            targetable: false,
            kind: BuildingKind::Ruin(RuinMarker {
                original_kind: original_kind.into(),
                rebuild_discount: 0.5,
            }),
        }
    }

    pub fn storage_data(&self) -> Option<&StorageBuilding> {
        match &self.kind {
            BuildingKind::Storage(storage) => Some(storage),
            _ => None,
        }
    }

    pub fn storage_data_mut(&mut self) -> Option<&mut StorageBuilding> {
        match &mut self.kind {
            BuildingKind::Storage(storage) => Some(storage),
            _ => None,
        }
    }

    pub fn production_data(&self) -> Option<&ProductionBuilding> {
        match &self.kind {
            BuildingKind::Production(production) => Some(production),
            _ => None,
        }
    }

    pub fn production_data_mut(&mut self) -> Option<&mut ProductionBuilding> {
        match &mut self.kind {
            BuildingKind::Production(production) => Some(production),
            _ => None,
        }
    }

    pub fn defense_data_mut(&mut self) -> Option<&mut DefenseBuilding> {
        match &mut self.kind {
            BuildingKind::Defense(defense) => Some(defense),
            _ => None,
        }
    }

    pub fn is_storage(&self) -> bool {
        matches!(self.kind, BuildingKind::Storage(_))
    }

    pub fn is_destructible_blocker(&self) -> bool {
        !self.walkable && self.targetable
    }

    pub fn is_destroyed(&self) -> bool {
        matches!(self.kind, BuildingKind::Ruin(_))
    }

    fn kind_name(&self) -> &'static str {
        match self.kind {
            BuildingKind::Production(_) => "production",
            BuildingKind::Storage(_) => "storage",
            BuildingKind::Defense(_) => "defense",
            BuildingKind::Obstacle(_) => "obstacle",
            BuildingKind::Ruin(_) => "ruin",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Worker {
    pub id: WorkerId,
    pub home_production: BuildingId,
    pub position: GridPos,
    pub carry_capacity: f64,
    pub carrying: Option<ResourceStack>,
    pub state: WorkerState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkerState {
    Idle,
    Delivering { storage_id: BuildingId },
    Returning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RaiderTemplate {
    pub name: String,
    pub max_hit_points: f64,
    pub speed: f64,
    pub attack_damage_per_second: f64,
    pub carry_capacity: f64,
    pub looting_rate: f64,
    pub drop_ratio_on_death: f64,
}

impl RaiderTemplate {
    pub fn small_fast() -> Self {
        Self {
            name: "small_fast".into(),
            max_hit_points: 20.0,
            speed: 1.5,
            attack_damage_per_second: 6.0,
            carry_capacity: 10.0,
            looting_rate: 2.0,
            drop_ratio_on_death: 0.75,
        }
    }

    pub fn normal() -> Self {
        Self {
            name: "normal".into(),
            max_hit_points: 35.0,
            speed: 1.0,
            attack_damage_per_second: 8.0,
            carry_capacity: 18.0,
            looting_rate: 2.0,
            drop_ratio_on_death: 0.75,
        }
    }

    pub fn bulky() -> Self {
        Self {
            name: "bulky".into(),
            max_hit_points: 60.0,
            speed: 0.8,
            attack_damage_per_second: 12.0,
            carry_capacity: 30.0,
            looting_rate: 2.0,
            drop_ratio_on_death: 0.9,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Raider {
    pub id: RaiderId,
    pub template_name: String,
    pub lane_id: String,
    pub position: GridPos,
    pub exit: GridPos,
    pub hit_points: f64,
    pub max_hit_points: f64,
    pub speed: f64,
    pub attack_damage_per_second: f64,
    pub carry_capacity: f64,
    pub looting_rate: f64,
    pub drop_ratio_on_death: f64,
    pub carried: BTreeMap<String, f64>,
    pub target_storage: Option<BuildingId>,
    pub target_blocker: Option<BuildingId>,
    pub state: RaiderState,
}

impl Raider {
    fn from_template(
        id: RaiderId,
        lane_id: impl Into<String>,
        spawn: GridPos,
        exit: GridPos,
        template: &RaiderTemplate,
    ) -> Self {
        Self {
            id,
            template_name: template.name.clone(),
            lane_id: lane_id.into(),
            position: spawn,
            exit,
            hit_points: template.max_hit_points,
            max_hit_points: template.max_hit_points,
            speed: template.speed,
            attack_damage_per_second: template.attack_damage_per_second,
            carry_capacity: template.carry_capacity,
            looting_rate: template.looting_rate,
            drop_ratio_on_death: template.drop_ratio_on_death,
            carried: BTreeMap::new(),
            target_storage: None,
            target_blocker: None,
            state: RaiderState::Spawned,
        }
    }

    pub fn total_carried(&self) -> f64 {
        self.carried.values().copied().sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RaiderState {
    Spawned,
    MovingToStorage,
    DestroyingBlocker,
    Looting,
    Retreating,
    Dead,
    Escaped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneDefinition {
    pub id: String,
    pub spawn: GridPos,
    pub exit: GridPos,
}

impl LaneDefinition {
    pub fn new(id: impl Into<String>, spawn: GridPos, exit: GridPos) -> Self {
        Self {
            id: id.into(),
            spawn,
            exit,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveDefinition {
    pub lane_id: String,
    pub start_delay: f64,
    pub count: usize,
    pub raider_type: RaiderTemplate,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RaidDefinition {
    pub name: String,
    pub waves: Vec<WaveDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RaidResult {
    pub raid_name: String,
    pub damaged_buildings: BTreeMap<BuildingId, f64>,
    pub destroyed_buildings: Vec<BuildingId>,
    pub surviving_buildings: Vec<BuildingId>,
    pub stolen_resources: BTreeMap<String, f64>,
    pub recovered_resources: BTreeMap<String, f64>,
    pub raiders_killed: usize,
    pub raiders_escaped: usize,
    pub blockers_destroyed: usize,
    pub defense_damage_dealt: f64,
    pub raid_path_lengths: Vec<f64>,
    pub raider_time_to_first_storage: Option<f64>,
}

impl RaidResult {
    fn new(raid_name: impl Into<String>) -> Self {
        Self {
            raid_name: raid_name.into(),
            damaged_buildings: BTreeMap::new(),
            destroyed_buildings: Vec::new(),
            surviving_buildings: Vec::new(),
            stolen_resources: BTreeMap::new(),
            recovered_resources: BTreeMap::new(),
            raiders_killed: 0,
            raiders_escaped: 0,
            blockers_destroyed: 0,
            defense_damage_dealt: 0.0,
            raid_path_lengths: Vec::new(),
            raider_time_to_first_storage: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EconomyTickResult {
    pub average_worker_delivery_path_cost: Option<f64>,
    pub production_output: BTreeMap<String, f64>,
    pub resources_delivered_to_storage: BTreeMap<String, f64>,
    pub resources_stuck_in_production_buffers: BTreeMap<BuildingId, f64>,
    pub storage_fill_levels: BTreeMap<BuildingId, f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugMetrics {
    pub average_worker_delivery_path_cost: Option<f64>,
    pub production_output_per_day: BTreeMap<String, f64>,
    pub resources_delivered_to_storage: BTreeMap<String, f64>,
    pub resources_stuck_in_production_buffers: BTreeMap<BuildingId, f64>,
    pub storage_fill_levels: BTreeMap<BuildingId, f64>,
    pub raid_path_lengths: Vec<f64>,
    pub raider_time_to_first_storage: Option<f64>,
    pub resources_stolen: BTreeMap<String, f64>,
    pub resources_recovered: BTreeMap<String, f64>,
    pub buildings_damaged: BTreeMap<BuildingId, f64>,
    pub buildings_destroyed: Vec<BuildingId>,
    pub blockers_destroyed: usize,
    pub raiders_killed: usize,
    pub raiders_escaped: usize,
    pub defense_damage_dealt: f64,
}

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PathActor {
    Worker,
    Raider,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockerHandling {
    Impassable,
    IgnoreDestructible,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PathOptions {
    pub actor: PathActor,
    pub blocker_handling: BlockerHandling,
}

impl PathOptions {
    pub fn worker() -> Self {
        Self {
            actor: PathActor::Worker,
            blocker_handling: BlockerHandling::Impassable,
        }
    }

    pub fn raider() -> Self {
        Self {
            actor: PathActor::Raider,
            blocker_handling: BlockerHandling::Impassable,
        }
    }

    pub fn raider_with_blockers() -> Self {
        Self {
            actor: PathActor::Raider,
            blocker_handling: BlockerHandling::IgnoreDestructible,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SettlementMoveProfile {
    Worker,
    Raider,
    RaiderCanBreakBlockers,
}

impl From<PathOptions> for SettlementMoveProfile {
    fn from(value: PathOptions) -> Self {
        match (value.actor, value.blocker_handling) {
            (PathActor::Worker, _) => Self::Worker,
            (PathActor::Raider, BlockerHandling::Impassable) => Self::Raider,
            (PathActor::Raider, BlockerHandling::IgnoreDestructible) => {
                Self::RaiderCanBreakBlockers
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathResult {
    pub found: bool,
    pub path: Vec<GridPos>,
    pub path_length: usize,
    pub total_movement_cost: f64,
}

impl PathResult {
    fn not_found() -> Self {
        Self {
            found: false,
            path: Vec::new(),
            path_length: 0,
            total_movement_cost: f64::INFINITY,
        }
    }

    pub fn next_step(&self) -> Option<GridPos> {
        self.path.get(1).copied()
    }
}

struct SettlementNavDomain<'a> {
    game: &'a SettlementGame,
    nav_version: u64,
}

impl<'a> SettlementNavDomain<'a> {
    fn new(game: &'a SettlementGame) -> Self {
        Self {
            game,
            nav_version: game.nav_version,
        }
    }
}

impl NavDomain for SettlementNavDomain<'_> {
    type Node = GridPos;
    type Profile = SettlementMoveProfile;

    fn neighbors(
        &self,
        node: Self::Node,
        profile: Self::Profile,
        out: &mut Vec<(Self::Node, f32)>,
    ) {
        out.clear();
        for neighbor in self.game.map.neighbors(node).into_iter().flatten() {
            let options = match profile {
                SettlementMoveProfile::Worker => PathOptions::worker(),
                SettlementMoveProfile::Raider => PathOptions::raider(),
                SettlementMoveProfile::RaiderCanBreakBlockers => {
                    PathOptions::raider_with_blockers()
                }
            };
            let Some(cost) = self.game.tile_movement_cost(neighbor, options) else {
                continue;
            };
            out.push((neighbor, cost as f32));
        }
    }

    fn heuristic(&self, from: Self::Node, goal: Self::Node, _: Self::Profile) -> f32 {
        from.manhattan_distance(goal) as f32 * MIN_MOVEMENT_COST as f32
    }

    fn version(&self) -> u64 {
        self.nav_version
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RaiderTarget {
    pub storage_id: BuildingId,
    pub path: PathResult,
    pub blocker_id: Option<BuildingId>,
    pub blocker_approach: Option<PathResult>,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Attacker {
    Raider(RaiderId),
}

#[derive(Clone, Debug, PartialEq)]
struct DefenseProjectile {
    kind: DefenseProjectileKind,
    source_id: BuildingId,
    target_raider_id: RaiderId,
    target_position: GridPos,
    damage: f64,
    remaining_time: f64,
    tracking: bool,
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

impl Default for DebugMetrics {
    fn default() -> Self {
        Self {
            average_worker_delivery_path_cost: None,
            production_output_per_day: BTreeMap::new(),
            resources_delivered_to_storage: BTreeMap::new(),
            resources_stuck_in_production_buffers: BTreeMap::new(),
            storage_fill_levels: BTreeMap::new(),
            raid_path_lengths: Vec::new(),
            raider_time_to_first_storage: None,
            resources_stolen: BTreeMap::new(),
            resources_recovered: BTreeMap::new(),
            buildings_damaged: BTreeMap::new(),
            buildings_destroyed: Vec::new(),
            blockers_destroyed: 0,
            raiders_killed: 0,
            raiders_escaped: 0,
            defense_damage_dealt: 0.0,
        }
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

    pub fn tick_economy(&mut self, delta_time: f64) -> EconomyTickResult {
        if self.active_raid.is_some() {
            return self.economy_tick_result();
        }
        let remaining_until_raid = (self.config.day_length - self.current_day_elapsed).max(0.0);
        let economy_time = delta_time.min(remaining_until_raid);
        self.step_economy(economy_time);
        self.current_day_elapsed += economy_time;
        if self.current_day_elapsed + EPSILON >= self.config.day_length {
            self.start_raid(self.default_raid_definition.clone());
        }
        self.economy_tick_result()
    }

    pub fn tick_raid(&mut self, delta_time: f64) -> Option<RaidResult> {
        self.active_raid.as_ref()?;
        let mut remaining = delta_time.max(0.0);
        while remaining > EPSILON {
            let step = remaining.min(self.config.simulation_step);
            remaining -= step;
            if let Some(active_raid) = self.active_raid.as_mut() {
                active_raid.elapsed += step;
            }
            self.spawn_due_waves();
            self.step_defenses(step);
            self.step_raiders(step);
            self.step_defense_projectiles(step);
        }
        self.maybe_end_raid()
    }

    pub fn start_raid(&mut self, raid_definition: RaidDefinition) {
        if self.active_raid.is_some() {
            return;
        }
        self.raiders.clear();
        self.raider_routes.clear();
        self.defense_projectiles.clear();
        self.last_raid_result = None;
        self.active_raid = Some(ActiveRaid {
            result: RaidResult::new(raid_definition.name.clone()),
            definition: raid_definition,
            elapsed: 0.0,
            spawned_waves: BTreeSet::new(),
        });
    }

    pub fn trigger_raid_early(&mut self) {
        self.start_raid(self.default_raid_definition.clone());
    }

    pub fn end_raid(&mut self) -> Option<RaidResult> {
        self.active_raid.as_ref()?;
        Some(self.finish_raid())
    }

    pub fn find_path(&self, start: GridPos, goal: GridPos, options: PathOptions) -> PathResult {
        if !self.map.in_bounds(start) || !self.map.in_bounds(goal) {
            return PathResult::not_found();
        }
        let nav_result = astar(
            &SettlementNavDomain::new(self),
            PathRequest {
                start,
                goal,
                profile: SettlementMoveProfile::from(options),
                allow_partial: false,
            },
        );
        PathResult {
            found: nav_result.found,
            path: nav_result.nodes,
            path_length: nav_result.step_count,
            total_movement_cost: nav_result.total_cost as f64,
        }
    }

    pub fn find_best_storage_for_production(
        &self,
        production_building: BuildingId,
    ) -> Option<(BuildingId, PathResult)> {
        let building = self.buildings.get(&production_building)?;
        let production = building.production_data()?;
        let mut best: Option<(BuildingId, PathResult)> = None;

        for (&storage_id, storage_building) in &self.buildings {
            let Some(storage) = storage_building.storage_data() else {
                continue;
            };
            if !storage.accepts(&production.resource_type) || storage.free_capacity() <= EPSILON {
                continue;
            }
            let path = self.find_path(
                building.position,
                storage_building.position,
                PathOptions::worker(),
            );
            if !path.found {
                continue;
            }
            match &best {
                Some((_, current)) if current.total_movement_cost <= path.total_movement_cost => {}
                _ => best = Some((storage_id, path)),
            }
        }

        best
    }

    pub fn find_best_storage_target_for_raider(&self, raider_id: RaiderId) -> Option<RaiderTarget> {
        let raider = self.raiders.get(&raider_id)?;
        let mut best_target: Option<RaiderTarget> = None;

        for (&storage_id, storage_building) in &self.buildings {
            let Some(storage) = storage_building.storage_data() else {
                continue;
            };
            let stored_value = storage.total_stored();
            let reachable_path = self.find_path(
                raider.position,
                storage_building.position,
                PathOptions::raider(),
            );
            let candidate = if reachable_path.found {
                RaiderTarget {
                    storage_id,
                    score: self.config.stored_value_weight * stored_value
                        - self.config.path_cost_weight * reachable_path.total_movement_cost,
                    path: reachable_path,
                    blocker_id: None,
                    blocker_approach: None,
                }
            } else {
                let pseudo_path = self.find_path(
                    raider.position,
                    storage_building.position,
                    PathOptions::raider_with_blockers(),
                );
                if !pseudo_path.found {
                    continue;
                }
                let Some((blocker_id, approach_path)) =
                    self.first_blocker_on_path_with_approach(raider.position, &pseudo_path)
                else {
                    continue;
                };
                RaiderTarget {
                    storage_id,
                    score: self.config.stored_value_weight * stored_value
                        - self.config.path_cost_weight * pseudo_path.total_movement_cost,
                    path: pseudo_path,
                    blocker_id: Some(blocker_id),
                    blocker_approach: Some(approach_path),
                }
            };

            match &best_target {
                Some(existing)
                    if existing.score > candidate.score + EPSILON
                        || ((existing.score - candidate.score).abs() <= EPSILON
                            && existing.path.total_movement_cost
                                <= candidate.path.total_movement_cost) => {}
                _ => best_target = Some(candidate),
            }
        }

        best_target
    }

    pub fn deliver_resources(
        &mut self,
        worker_id: WorkerId,
        production_building: BuildingId,
        storage_building: BuildingId,
    ) -> f64 {
        let carrying = self
            .workers
            .get(&worker_id)
            .and_then(|worker| worker.carrying.clone());
        let Some(carrying) = carrying else {
            return 0.0;
        };

        let delivered = if let Some(storage) = self
            .buildings
            .get_mut(&storage_building)
            .and_then(Building::storage_data_mut)
        {
            let accepted_amount = if storage.accepts(&carrying.resource) {
                carrying.amount.min(storage.free_capacity())
            } else {
                0.0
            };
            if accepted_amount > 0.0 {
                *storage.stored.entry(carrying.resource.clone()).or_default() += accepted_amount;
            }
            accepted_amount
        } else {
            0.0
        };

        if let Some(worker) = self.workers.get_mut(&worker_id) {
            if delivered + EPSILON >= carrying.amount {
                worker.carrying = None;
            } else {
                worker.carrying = Some(ResourceStack::new(
                    carrying.resource.clone(),
                    carrying.amount - delivered,
                ));
            }
            worker.state = WorkerState::Returning;
        }

        if delivered > 0.0 {
            *self
                .debug
                .resources_delivered_to_storage
                .entry(carrying.resource)
                .or_default() += delivered;
        }

        let _ = production_building;
        delivered
    }

    pub fn loot_storage(
        &mut self,
        raider_id: RaiderId,
        storage_building: BuildingId,
        delta_time: f64,
    ) -> f64 {
        let (rate, remaining_capacity) = match self.raiders.get(&raider_id) {
            Some(raider) => (
                raider.looting_rate,
                raider.carry_capacity - raider.total_carried(),
            ),
            None => return 0.0,
        };
        if remaining_capacity <= EPSILON {
            return 0.0;
        }
        let amount_to_loot = rate * delta_time.min(self.config.simulation_step);
        let mut looted = 0.0;
        let mut looted_resources = BTreeMap::new();

        if let Some(storage) = self
            .buildings
            .get_mut(&storage_building)
            .and_then(Building::storage_data_mut)
        {
            let mut budget = remaining_capacity.min(amount_to_loot);
            let keys: Vec<String> = storage.stored.keys().cloned().collect();
            for key in keys {
                if budget <= EPSILON {
                    break;
                }
                let Some(amount) = storage.stored.get_mut(&key) else {
                    continue;
                };
                let taken = (*amount).min(budget);
                if taken > 0.0 {
                    *amount -= taken;
                    budget -= taken;
                    looted += taken;
                    looted_resources.insert(key, taken);
                }
            }
            storage.stored.retain(|_, amount| *amount > EPSILON);
        }

        if looted <= EPSILON {
            return 0.0;
        }

        if let Some(raider) = self.raiders.get_mut(&raider_id) {
            for (resource, amount) in looted_resources {
                *raider.carried.entry(resource.clone()).or_default() += amount;
                *self
                    .debug
                    .resources_stolen
                    .entry(resource.clone())
                    .or_default() += amount;
                if let Some(raid) = self.active_raid.as_mut() {
                    *raid.result.stolen_resources.entry(resource).or_default() += amount;
                }
            }
        }

        looted
    }

    pub fn damage_building(
        &mut self,
        attacker: Attacker,
        building_id: BuildingId,
        delta_time: f64,
    ) -> f64 {
        let damage = match attacker {
            Attacker::Raider(raider_id) => self
                .raiders
                .get(&raider_id)
                .map(|raider| raider.attack_damage_per_second * delta_time)
                .unwrap_or(0.0),
        };
        if damage <= EPSILON {
            return 0.0;
        }
        let mut destroyed = false;
        let mut actual_damage = 0.0;

        if let Some(building) = self.buildings.get_mut(&building_id) {
            if building.is_destroyed() {
                return 0.0;
            }
            actual_damage = damage.min(building.hit_points);
            building.hit_points = (building.hit_points - damage).max(0.0);
            if let BuildingKind::Obstacle(obstacle) = &mut building.kind {
                obstacle.hit_points = building.hit_points;
            }
            *self.debug.buildings_damaged.entry(building_id).or_default() += actual_damage;
            if let Some(raid) = self.active_raid.as_mut() {
                *raid
                    .result
                    .damaged_buildings
                    .entry(building_id)
                    .or_default() += actual_damage;
            }
            destroyed = building.hit_points <= EPSILON;
        }

        if destroyed {
            self.destroy_building(building_id);
        }

        actual_damage
    }

    pub fn recalculate_paths_after_map_change(&mut self) {
        self.paths_dirty = true;
        self.nav_version = self.nav_version.wrapping_add(1);
    }

    pub fn get_debug_metrics(&self) -> DebugMetrics {
        let mut metrics = self.debug.clone();
        metrics.average_worker_delivery_path_cost = self.average_delivery_path_cost();
        metrics.resources_stuck_in_production_buffers = self.current_buffer_levels();
        metrics.storage_fill_levels = self.current_storage_fill_levels();
        metrics
    }

    pub fn current_raid(&self) -> Option<&RaidDefinition> {
        self.active_raid.as_ref().map(|raid| &raid.definition)
    }

    pub fn last_raid_result(&self) -> Option<&RaidResult> {
        self.last_raid_result.as_ref()
    }

    fn step_economy(&mut self, delta_time: f64) {
        let mut remaining = delta_time.max(0.0);
        while remaining > EPSILON {
            let step = remaining.min(self.config.simulation_step);
            remaining -= step;
            self.step_production(step);
            self.step_workers(step);
        }
    }

    fn step_production(&mut self, delta_time: f64) {
        for building in self.buildings.values_mut() {
            let Some(production) = building.production_data_mut() else {
                continue;
            };
            let available_space =
                (production.output_buffer_capacity - production.output_buffer.amount).max(0.0);
            if available_space <= EPSILON {
                continue;
            }
            let produced =
                (production.generation_rate_per_second * delta_time).min(available_space);
            if produced > 0.0 {
                production.output_buffer.amount += produced;
                *self
                    .debug
                    .production_output_per_day
                    .entry(production.resource_type.clone())
                    .or_default() += produced;
            }
        }
    }

    fn step_workers(&mut self, delta_time: f64) {
        let worker_ids: Vec<WorkerId> = self.workers.keys().copied().collect();
        for worker_id in worker_ids {
            if self.paths_dirty {
                self.worker_routes.remove(&worker_id);
            }
            self.step_worker(worker_id, delta_time);
        }
        self.paths_dirty = false;
    }

    fn step_worker(&mut self, worker_id: WorkerId, delta_time: f64) {
        let Some(worker) = self.workers.get(&worker_id).cloned() else {
            return;
        };
        match worker.state {
            WorkerState::Idle => self.try_dispatch_worker(worker_id),
            WorkerState::Delivering { storage_id } => {
                let reached = self.advance_unit_route(
                    delta_time,
                    worker.position,
                    worker_id,
                    PathOptions::worker(),
                    false,
                );
                if reached {
                    let home = worker.home_production;
                    self.deliver_resources(worker_id, home, storage_id);
                    self.assign_worker_return_route(worker_id);
                }
            }
            WorkerState::Returning => {
                let reached = self.advance_unit_route(
                    delta_time,
                    worker.position,
                    worker_id,
                    PathOptions::worker(),
                    false,
                );
                if reached {
                    if let Some(worker) = self.workers.get_mut(&worker_id) {
                        worker.state = WorkerState::Idle;
                        worker.position = self
                            .buildings
                            .get(&worker.home_production)
                            .map(|building| building.position)
                            .unwrap_or(worker.position);
                    }
                    self.worker_routes.remove(&worker_id);
                }
            }
        }
    }

    fn try_dispatch_worker(&mut self, worker_id: WorkerId) {
        let Some(worker) = self.workers.get(&worker_id).cloned() else {
            return;
        };
        let Some((storage_id, path)) =
            self.find_best_storage_for_production(worker.home_production)
        else {
            return;
        };
        let Some((resource_type, available)) = self
            .buildings
            .get(&worker.home_production)
            .and_then(Building::production_data)
            .map(|production| {
                (
                    production.resource_type.clone(),
                    production.output_buffer.amount.min(worker.carry_capacity),
                )
            })
        else {
            return;
        };
        if available <= EPSILON {
            return;
        }

        let free_capacity = self
            .buildings
            .get(&storage_id)
            .and_then(Building::storage_data)
            .map(StorageBuilding::free_capacity)
            .unwrap_or(0.0);
        let picked_up = available.min(free_capacity);
        if picked_up <= EPSILON {
            return;
        }

        if let Some(production) = self
            .buildings
            .get_mut(&worker.home_production)
            .and_then(Building::production_data_mut)
        {
            production.output_buffer.amount -= picked_up;
        }

        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.carrying = Some(ResourceStack::new(resource_type, picked_up));
            worker.state = WorkerState::Delivering { storage_id };
        }
        self.worker_routes
            .insert(worker_id, RouteProgress::from_path(&path).unwrap());
        self.delivery_path_cost_sum += path.total_movement_cost;
        self.delivery_path_cost_count += 1;
    }

    fn assign_worker_return_route(&mut self, worker_id: WorkerId) {
        let Some(worker) = self.workers.get(&worker_id).cloned() else {
            return;
        };
        let Some(home_position) = self
            .buildings
            .get(&worker.home_production)
            .map(|building| building.position)
        else {
            return;
        };
        let path = self.find_path(worker.position, home_position, PathOptions::worker());
        if let Some(route) = RouteProgress::from_path(&path) {
            self.worker_routes.insert(worker_id, route);
        }
    }

    fn spawn_due_waves(&mut self) {
        let Some(active_raid) = self.active_raid.as_mut() else {
            return;
        };
        let elapsed = active_raid.elapsed;
        let mut to_spawn = Vec::new();
        for (index, wave) in active_raid.definition.waves.iter().enumerate() {
            if active_raid.spawned_waves.contains(&index) || wave.start_delay > elapsed + EPSILON {
                continue;
            }
            to_spawn.push(index);
        }

        for index in to_spawn {
            active_raid.spawned_waves.insert(index);
            let wave = active_raid.definition.waves[index].clone();
            let Some(lane) = self.map.lanes.get(&wave.lane_id).cloned() else {
                continue;
            };
            for _ in 0..wave.count {
                let raider_id = self.next_raider_id;
                self.next_raider_id += 1;
                self.raiders.insert(
                    raider_id,
                    Raider::from_template(
                        raider_id,
                        lane.id.clone(),
                        lane.spawn,
                        lane.exit,
                        &wave.raider_type,
                    ),
                );
            }
        }
    }

    fn step_defenses(&mut self, delta_time: f64) {
        let defense_ids: Vec<BuildingId> = self
            .buildings
            .iter()
            .filter(|(_, building)| matches!(building.kind, BuildingKind::Defense(_)))
            .map(|(id, _)| *id)
            .collect();
        for defense_id in defense_ids {
            let Some((priority, weapons)) =
                self.buildings
                    .get(&defense_id)
                    .and_then(|building| match &building.kind {
                        BuildingKind::Defense(defense) => {
                            Some((defense.targeting_priority, defense.weapons.clone()))
                        }
                        _ => None,
                    })
            else {
                continue;
            };
            for (weapon_index, weapon) in weapons.iter().copied().enumerate() {
                let updated_cooldown = (weapon.cooldown_remaining - delta_time).max(0.0);
                if let Some(defense) = self
                    .buildings
                    .get_mut(&defense_id)
                    .and_then(Building::defense_data_mut)
                {
                    if let Some(slot) = defense.weapons.get_mut(weapon_index) {
                        slot.cooldown_remaining = updated_cooldown;
                    }
                }
                if updated_cooldown > EPSILON {
                    continue;
                }
                let Some(target_id) =
                    self.select_defense_target(defense_id, weapon.range, priority)
                else {
                    continue;
                };
                self.launch_defense_projectile(defense_id, target_id, weapon);
                if let Some(defense) = self
                    .buildings
                    .get_mut(&defense_id)
                    .and_then(Building::defense_data_mut)
                {
                    if let Some(slot) = defense.weapons.get_mut(weapon_index) {
                        slot.cooldown_remaining = weapon.attack_cooldown;
                    }
                }
            }
        }
    }

    fn launch_defense_projectile(
        &mut self,
        defense_id: BuildingId,
        target_raider_id: RaiderId,
        weapon: DefenseWeapon,
    ) {
        let Some(source_position) = self
            .buildings
            .get(&defense_id)
            .map(|building| building.position)
        else {
            return;
        };
        let Some(target_position) = self
            .raiders
            .get(&target_raider_id)
            .map(|raider| raider.position)
        else {
            return;
        };
        let travel_distance = source_position.manhattan_distance(target_position).max(1) as f64;
        let remaining_time = (travel_distance / weapon.projectile_speed).max(EPSILON);
        self.defense_projectiles.push(DefenseProjectile {
            kind: weapon.projectile_kind,
            source_id: defense_id,
            target_raider_id,
            target_position,
            damage: weapon.damage,
            remaining_time,
            tracking: weapon.tracking,
        });
    }

    fn step_defense_projectiles(&mut self, delta_time: f64) {
        let mut remaining_projectiles = Vec::with_capacity(self.defense_projectiles.len());
        for mut projectile in std::mem::take(&mut self.defense_projectiles) {
            projectile.remaining_time -= delta_time;
            if projectile.remaining_time > EPSILON {
                remaining_projectiles.push(projectile);
                continue;
            }
            self.resolve_defense_projectile(projectile);
        }
        self.defense_projectiles = remaining_projectiles;
    }

    fn resolve_defense_projectile(&mut self, projectile: DefenseProjectile) {
        let Some(raider) = self.raiders.get(&projectile.target_raider_id) else {
            return;
        };
        if matches!(raider.state, RaiderState::Dead | RaiderState::Escaped) {
            return;
        }
        let should_hit = projectile.tracking || raider.position == projectile.target_position;
        if !should_hit {
            return;
        }
        self.apply_defense_damage(projectile.target_raider_id, projectile.damage);
    }

    fn apply_defense_damage(&mut self, target_id: RaiderId, damage: f64) {
        if let Some(raider) = self.raiders.get_mut(&target_id) {
            raider.hit_points -= damage;
            self.debug.defense_damage_dealt += damage;
            if let Some(active_raid) = self.active_raid.as_mut() {
                active_raid.result.defense_damage_dealt += damage;
            }
            if raider.hit_points <= EPSILON {
                let _ = raider;
                self.kill_raider(target_id);
            }
        }
    }

    fn select_defense_target(
        &self,
        defense_id: BuildingId,
        range: i32,
        priority: DefenseTargetPriority,
    ) -> Option<RaiderId> {
        let defense_position = self.buildings.get(&defense_id)?.position;
        let mut targets: Vec<&Raider> = self
            .raiders
            .values()
            .filter(|raider| {
                !matches!(raider.state, RaiderState::Dead | RaiderState::Escaped)
                    && defense_position.manhattan_distance(raider.position) <= range
            })
            .collect();
        targets.sort_by(|left, right| match priority {
            DefenseTargetPriority::FirstInRange => left.id.cmp(&right.id),
            DefenseTargetPriority::LowestHealth => left.hit_points.total_cmp(&right.hit_points),
        });
        targets.first().map(|raider| raider.id)
    }

    fn step_raiders(&mut self, delta_time: f64) {
        let raider_ids: Vec<RaiderId> = self.raiders.keys().copied().collect();
        for raider_id in raider_ids {
            if self.paths_dirty {
                self.raider_routes.remove(&raider_id);
            }
            self.step_raider(raider_id, delta_time);
        }
        self.paths_dirty = false;
    }

    fn step_raider(&mut self, raider_id: RaiderId, delta_time: f64) {
        let Some(raider) = self.raiders.get(&raider_id).cloned() else {
            return;
        };
        match raider.state {
            RaiderState::Spawned | RaiderState::MovingToStorage => {
                if (matches!(raider.state, RaiderState::Spawned)
                    || !self.raider_routes.contains_key(&raider_id)
                    || self.paths_dirty)
                    && !self.ensure_raider_storage_target(raider_id)
                {
                    return;
                }
                let reached = self.advance_unit_route(
                    delta_time,
                    raider.position,
                    raider_id,
                    PathOptions::raider(),
                    true,
                );
                if reached {
                    if let Some(raider) = self.raiders.get_mut(&raider_id) {
                        raider.state = RaiderState::Looting;
                    }
                    if self.debug.raider_time_to_first_storage.is_none() {
                        let elapsed = self.active_raid.as_ref().map(|raid| raid.elapsed);
                        self.debug.raider_time_to_first_storage = elapsed;
                        if let (Some(active_raid), Some(elapsed)) =
                            (self.active_raid.as_mut(), elapsed)
                        {
                            active_raid.result.raider_time_to_first_storage = Some(elapsed);
                        }
                    }
                }
            }
            RaiderState::DestroyingBlocker => {
                let blocker_id = match self
                    .raiders
                    .get(&raider_id)
                    .and_then(|raider| raider.target_blocker)
                {
                    Some(blocker_id) => blocker_id,
                    None => {
                        self.ensure_raider_storage_target(raider_id);
                        return;
                    }
                };
                if !self.raider_routes.contains_key(&raider_id) || self.paths_dirty {
                    self.ensure_raider_storage_target(raider_id);
                }
                let reached = self.advance_unit_route(
                    delta_time,
                    raider.position,
                    raider_id,
                    PathOptions::raider(),
                    true,
                );
                if reached || self.is_adjacent_to_building(raider_id, blocker_id) {
                    self.damage_building(Attacker::Raider(raider_id), blocker_id, delta_time);
                    if self
                        .buildings
                        .get(&blocker_id)
                        .is_none_or(Building::is_destroyed)
                    {
                        if let Some(raider) = self.raiders.get_mut(&raider_id) {
                            raider.target_blocker = None;
                            raider.state = RaiderState::MovingToStorage;
                        }
                        self.raider_routes.remove(&raider_id);
                        self.recalculate_paths_after_map_change();
                    }
                }
            }
            RaiderState::Looting => {
                let Some(storage_id) = self
                    .raiders
                    .get(&raider_id)
                    .and_then(|raider| raider.target_storage)
                else {
                    self.ensure_raider_storage_target(raider_id);
                    return;
                };
                let looted = self.loot_storage(raider_id, storage_id, delta_time);
                let should_retreat = self
                    .raiders
                    .get(&raider_id)
                    .map(|raider| raider.total_carried() + EPSILON >= raider.carry_capacity)
                    .unwrap_or(true)
                    || self
                        .buildings
                        .get(&storage_id)
                        .and_then(Building::storage_data)
                        .map(|storage| storage.total_stored() <= EPSILON)
                        .unwrap_or(true)
                    || looted <= EPSILON;
                if should_retreat {
                    self.assign_raider_retreat_route(raider_id);
                }
            }
            RaiderState::Retreating => {
                let escaped = self.advance_unit_route(
                    delta_time,
                    raider.position,
                    raider_id,
                    PathOptions::raider(),
                    true,
                );
                if escaped {
                    if let Some(raider) = self.raiders.get_mut(&raider_id) {
                        raider.state = RaiderState::Escaped;
                    }
                    self.debug.raiders_escaped += 1;
                    if let Some(active_raid) = self.active_raid.as_mut() {
                        active_raid.result.raiders_escaped += 1;
                    }
                }
            }
            RaiderState::Dead | RaiderState::Escaped => {}
        }
    }

    fn ensure_raider_storage_target(&mut self, raider_id: RaiderId) -> bool {
        let Some(target) = self.find_best_storage_target_for_raider(raider_id) else {
            if let Some(raider) = self.raiders.get_mut(&raider_id) {
                raider.state = RaiderState::Retreating;
            }
            self.assign_raider_retreat_route(raider_id);
            return false;
        };
        if let Some(raider) = self.raiders.get_mut(&raider_id) {
            raider.target_storage = Some(target.storage_id);
            raider.target_blocker = target.blocker_id;
            if let Some(blocker_id) = target.blocker_id {
                raider.state = RaiderState::DestroyingBlocker;
                if let Some(approach) = target
                    .blocker_approach
                    .as_ref()
                    .and_then(RouteProgress::from_path)
                {
                    self.raider_routes.insert(raider_id, approach);
                }
                let _ = blocker_id;
            } else {
                raider.state = RaiderState::MovingToStorage;
                if let Some(route) = RouteProgress::from_path(&target.path) {
                    self.raider_routes.insert(raider_id, route);
                }
            }
        }
        self.debug
            .raid_path_lengths
            .push(target.path.total_movement_cost);
        if let Some(active_raid) = self.active_raid.as_mut() {
            active_raid
                .result
                .raid_path_lengths
                .push(target.path.total_movement_cost);
        }
        true
    }

    fn assign_raider_retreat_route(&mut self, raider_id: RaiderId) {
        let Some((position, exit)) = self
            .raiders
            .get(&raider_id)
            .map(|raider| (raider.position, raider.exit))
        else {
            return;
        };
        if let Some(raider) = self.raiders.get_mut(&raider_id) {
            raider.state = RaiderState::Retreating;
        }
        let path = self.find_path(position, exit, PathOptions::raider());
        if let Some(route) = RouteProgress::from_path(&path) {
            self.raider_routes.insert(raider_id, route);
        }
    }

    fn advance_unit_route(
        &mut self,
        delta_time: f64,
        fallback_position: GridPos,
        unit_id: u32,
        path_options: PathOptions,
        is_raider: bool,
    ) -> bool {
        let Some(mut route) = (if is_raider {
            self.raider_routes.get(&unit_id).cloned()
        } else {
            self.worker_routes.get(&unit_id).cloned()
        }) else {
            return false;
        };
        let speed = if is_raider {
            self.raiders
                .get(&unit_id)
                .map(|raider| raider.speed)
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let mut remaining_budget = speed * delta_time;
        let mut current_position = if is_raider {
            self.raiders
                .get(&unit_id)
                .map(|raider| raider.position)
                .unwrap_or(fallback_position)
        } else {
            self.workers
                .get(&unit_id)
                .map(|worker| worker.position)
                .unwrap_or(fallback_position)
        };

        while remaining_budget > EPSILON && !route.is_finished() {
            let next_pos = route.path[route.next_index + 1];
            let Some(edge_cost) = self.tile_movement_cost(next_pos, path_options) else {
                return false;
            };
            let remaining_edge_cost = edge_cost - route.progress_on_edge;
            if remaining_budget + EPSILON >= remaining_edge_cost {
                remaining_budget -= remaining_edge_cost;
                route.progress_on_edge = 0.0;
                route.next_index += 1;
                current_position = next_pos;
            } else {
                route.progress_on_edge += remaining_budget;
                remaining_budget = 0.0;
            }
        }

        if is_raider {
            if let Some(raider) = self.raiders.get_mut(&unit_id) {
                raider.position = current_position;
            }
            self.raider_routes.insert(unit_id, route.clone());
        } else if let Some(worker) = self.workers.get_mut(&unit_id) {
            worker.position = current_position;
            self.worker_routes.insert(unit_id, route.clone());
        }

        route.is_finished()
    }

    fn tile_movement_cost(&self, pos: GridPos, options: PathOptions) -> Option<f64> {
        let tile = self.map.tile(pos);
        let base_cost = tile
            .road
            .as_ref()
            .map(|road| road.movement_cost)
            .or_else(|| tile.terrain.movement_cost())?;
        match tile.building {
            Some(building_id) => {
                let building = self.buildings.get(&building_id)?;
                if building.walkable {
                    Some(base_cost)
                } else if matches!(
                    options.blocker_handling,
                    BlockerHandling::IgnoreDestructible
                ) && building.is_destructible_blocker()
                {
                    Some(base_cost + BLOCKER_PENALTY_COST)
                } else {
                    None
                }
            }
            None => Some(base_cost),
        }
    }

    fn first_blocker_on_path_with_approach(
        &self,
        start: GridPos,
        pseudo_path: &PathResult,
    ) -> Option<(BuildingId, PathResult)> {
        for window in pseudo_path.path.windows(2) {
            let current = window[0];
            let next = window[1];
            let Some(building_id) = self.map.tile(next).building else {
                continue;
            };
            let Some(building) = self.buildings.get(&building_id) else {
                continue;
            };
            if !building.is_destructible_blocker() {
                continue;
            }
            let approach = self.find_path(start, current, PathOptions::raider());
            return Some((building_id, approach));
        }
        None
    }

    fn is_adjacent_to_building(&self, raider_id: RaiderId, building_id: BuildingId) -> bool {
        let Some(raider) = self.raiders.get(&raider_id) else {
            return false;
        };
        let Some(building) = self.buildings.get(&building_id) else {
            return false;
        };
        raider.position == building.position
            || raider.position.manhattan_distance(building.position) == 1
    }

    fn kill_raider(&mut self, raider_id: RaiderId) {
        let Some(raider) = self.raiders.get_mut(&raider_id) else {
            return;
        };
        if matches!(raider.state, RaiderState::Dead | RaiderState::Escaped) {
            return;
        }
        raider.state = RaiderState::Dead;
        self.debug.raiders_killed += 1;
        if let Some(active_raid) = self.active_raid.as_mut() {
            active_raid.result.raiders_killed += 1;
        }

        let dropped: Vec<(String, f64)> = raider
            .carried
            .iter()
            .map(|(resource, amount)| (resource.clone(), amount * raider.drop_ratio_on_death))
            .collect();
        for (resource, amount) in dropped {
            if amount <= EPSILON {
                continue;
            }
            *self.recovery_pool.entry(resource.clone()).or_default() += amount;
            *self
                .debug
                .resources_recovered
                .entry(resource.clone())
                .or_default() += amount;
            if let Some(active_raid) = self.active_raid.as_mut() {
                *active_raid
                    .result
                    .recovered_resources
                    .entry(resource)
                    .or_default() += amount;
            }
        }
        raider.carried.clear();
        self.raider_routes.remove(&raider_id);
    }

    fn destroy_building(&mut self, building_id: BuildingId) {
        let Some(building) = self.buildings.get_mut(&building_id) else {
            return;
        };
        if building.is_destroyed() {
            return;
        }
        let original_kind = building.kind_name().to_string();
        let position = building.position;
        *building = Building::ruin(position, original_kind);
        building.id = building_id;
        self.debug.buildings_destroyed.push(building_id);
        self.debug.blockers_destroyed += 1;
        if let Some(active_raid) = self.active_raid.as_mut() {
            active_raid.result.destroyed_buildings.push(building_id);
            active_raid.result.blockers_destroyed += 1;
        }
        self.recalculate_paths_after_map_change();
    }

    fn maybe_end_raid(&mut self) -> Option<RaidResult> {
        let active = self.active_raid.as_ref()?;
        let all_waves_spawned = active.spawned_waves.len() == active.definition.waves.len();
        let no_active_raiders = self
            .raiders
            .values()
            .all(|raider| matches!(raider.state, RaiderState::Dead | RaiderState::Escaped));
        (all_waves_spawned && no_active_raiders).then(|| self.finish_raid())
    }

    fn finish_raid(&mut self) -> RaidResult {
        let mut active_raid = self.active_raid.take().expect("raid must exist");
        active_raid.result.surviving_buildings = self
            .buildings
            .iter()
            .filter(|(_, building)| !building.is_destroyed())
            .map(|(id, _)| *id)
            .collect();
        self.current_day_elapsed = 0.0;
        self.raider_routes.clear();
        self.defense_projectiles.clear();
        self.raiders
            .retain(|_, raider| matches!(raider.state, RaiderState::Dead));
        self.last_raid_result = Some(active_raid.result.clone());
        active_raid.result
    }

    fn average_delivery_path_cost(&self) -> Option<f64> {
        (self.delivery_path_cost_count > 0)
            .then_some(self.delivery_path_cost_sum / self.delivery_path_cost_count as f64)
    }

    fn current_buffer_levels(&self) -> BTreeMap<BuildingId, f64> {
        self.buildings
            .iter()
            .filter_map(|(id, building)| {
                building
                    .production_data()
                    .map(|production| (*id, production.output_buffer.amount))
            })
            .collect()
    }

    fn current_storage_fill_levels(&self) -> BTreeMap<BuildingId, f64> {
        self.buildings
            .iter()
            .filter_map(|(id, building)| {
                building
                    .storage_data()
                    .map(|storage| (*id, storage.total_stored()))
            })
            .collect()
    }

    fn economy_tick_result(&self) -> EconomyTickResult {
        EconomyTickResult {
            average_worker_delivery_path_cost: self.average_delivery_path_cost(),
            production_output: self.debug.production_output_per_day.clone(),
            resources_delivered_to_storage: self.debug.resources_delivered_to_storage.clone(),
            resources_stuck_in_production_buffers: self.current_buffer_levels(),
            storage_fill_levels: self.current_storage_fill_levels(),
        }
    }

    #[allow(non_snake_case)]
    pub fn tickEconomy(&mut self, delta_time: f64) -> EconomyTickResult {
        self.tick_economy(delta_time)
    }

    #[allow(non_snake_case)]
    pub fn tickRaid(&mut self, delta_time: f64) -> Option<RaidResult> {
        self.tick_raid(delta_time)
    }

    #[allow(non_snake_case)]
    pub fn startRaid(&mut self, raid_definition: RaidDefinition) {
        self.start_raid(raid_definition);
    }

    #[allow(non_snake_case)]
    pub fn triggerRaidEarly(&mut self) {
        self.trigger_raid_early();
    }

    #[allow(non_snake_case)]
    pub fn endRaid(&mut self) -> Option<RaidResult> {
        self.end_raid()
    }

    #[allow(non_snake_case)]
    pub fn findPath(&self, start: GridPos, goal: GridPos, options: PathOptions) -> PathResult {
        self.find_path(start, goal, options)
    }

    #[allow(non_snake_case)]
    pub fn findBestStorageForProduction(
        &self,
        production_building: BuildingId,
    ) -> Option<(BuildingId, PathResult)> {
        self.find_best_storage_for_production(production_building)
    }

    #[allow(non_snake_case)]
    pub fn findBestStorageTargetForRaider(&self, raider_id: RaiderId) -> Option<RaiderTarget> {
        self.find_best_storage_target_for_raider(raider_id)
    }

    #[allow(non_snake_case)]
    pub fn deliverResources(
        &mut self,
        worker_id: WorkerId,
        production_building: BuildingId,
        storage_building: BuildingId,
    ) -> f64 {
        self.deliver_resources(worker_id, production_building, storage_building)
    }

    #[allow(non_snake_case)]
    pub fn lootStorage(
        &mut self,
        raider_id: RaiderId,
        storage_building: BuildingId,
        delta_time: f64,
    ) -> f64 {
        self.loot_storage(raider_id, storage_building, delta_time)
    }

    #[allow(non_snake_case)]
    pub fn damageBuilding(
        &mut self,
        attacker: Attacker,
        building_id: BuildingId,
        delta_time: f64,
    ) -> f64 {
        self.damage_building(attacker, building_id, delta_time)
    }

    #[allow(non_snake_case)]
    pub fn recalculatePathsAfterMapChange(&mut self) {
        self.recalculate_paths_after_map_change();
    }

    #[allow(non_snake_case)]
    pub fn getDebugMetrics(&self) -> DebugMetrics {
        self.get_debug_metrics()
    }
}

fn default_raid_definition() -> RaidDefinition {
    RaidDefinition {
        name: "default_night_raid".into(),
        waves: vec![
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 0.0,
                count: 2,
                raider_type: RaiderTemplate::small_fast(),
                name: Some("scouts".into()),
            },
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 12.0,
                count: 2,
                raider_type: RaiderTemplate::normal(),
                name: Some("main_force".into()),
            },
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 24.0,
                count: 1,
                raider_type: RaiderTemplate::bulky(),
                name: Some("bruiser".into()),
            },
        ],
    }
}
