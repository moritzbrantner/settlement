use super::SettlementGame;
use crate::{
    Attacker, BuildingId, DebugMetrics, EconomyTickResult, GridPos, PathOptions, PathResult,
    RaidDefinition, RaidResult, RaiderId, RaiderTarget, WorkerId,
};

impl SettlementGame {
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
