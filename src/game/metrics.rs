use super::SettlementGame;
use crate::{BuildingId, DebugMetrics, EconomyTickResult};
use std::collections::BTreeMap;

impl SettlementGame {
    pub fn get_debug_metrics(&self) -> DebugMetrics {
        let mut metrics = self.debug.clone();
        metrics.average_worker_delivery_path_cost = self.average_delivery_path_cost();
        metrics.resources_stuck_in_production_buffers = self.current_buffer_levels();
        metrics.storage_fill_levels = self.current_storage_fill_levels();
        metrics
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

    pub(super) fn economy_tick_result(&self) -> EconomyTickResult {
        EconomyTickResult {
            average_worker_delivery_path_cost: self.average_delivery_path_cost(),
            production_output: self.debug.production_output_per_day.clone(),
            resources_delivered_to_storage: self.debug.resources_delivered_to_storage.clone(),
            resources_stuck_in_production_buffers: self.current_buffer_levels(),
            storage_fill_levels: self.current_storage_fill_levels(),
        }
    }
}
