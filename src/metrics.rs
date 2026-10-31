use crate::BuildingId;
use std::collections::BTreeMap;

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
