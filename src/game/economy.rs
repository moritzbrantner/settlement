use super::{RouteProgress, SettlementGame};
use crate::{
    Building, BuildingId, EPSILON, EconomyTickResult, PathOptions, PathResult, ResourceStack,
    StorageBuilding, WorkerId, WorkerState,
};

impl SettlementGame {
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
}
