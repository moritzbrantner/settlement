use crate::{BuildingId, GridPos, ResourceStack};
use std::collections::BTreeMap;

pub type WorkerId = u32;
pub type RaiderId = u32;

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
    pub(crate) fn from_template(
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
