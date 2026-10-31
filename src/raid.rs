use crate::{BuildingId, GridPos, PathResult, RaiderId, RaiderTemplate};
use std::collections::BTreeMap;

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
    pub(crate) fn new(raid_name: impl Into<String>) -> Self {
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

pub(crate) fn default_raid_definition() -> RaidDefinition {
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
