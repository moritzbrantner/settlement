mod buildings;
mod game;
mod map;
mod metrics;
mod pathing;
mod raid;
mod units;
mod worldgen;

pub use buildings::{
    Building, BuildingId, BuildingKind, DefenseBuilding, DefenseProjectileKind,
    DefenseTargetPriority, DefenseWeapon, Obstacle, ObstacleBuilding, ProductionBuilding,
    RuinMarker, StorageBuilding, Wall,
};
pub use game::{SettlementGame, SimulationConfig};
pub use map::{GameMap, GridPos, ResourceStack, Road, TerrainType, Tile, TileMap};
pub use metrics::{DebugMetrics, EconomyTickResult};
pub use pathing::{BlockerHandling, PathActor, PathOptions, PathResult, SettlementMoveProfile};
pub use raid::{
    Attacker, LaneDefinition, RaidDefinition, RaidResult, RaiderTarget, WaveDefinition,
};
pub use units::{Raider, RaiderId, RaiderState, RaiderTemplate, Worker, WorkerId, WorkerState};
pub use worldgen::{
    GeneratedSettlementMap, PlayabilityReport, ResourceRegion, ResourceRegionKind,
    SettlementMapSpec, generate_playable_map, validate_playable_map,
};

const EPSILON: f64 = 0.000_001;
