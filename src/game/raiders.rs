use super::{ActiveRaid, RouteProgress, SettlementGame};
use crate::{
    Attacker, Building, BuildingId, BuildingKind, EPSILON, GridPos, PathOptions, PathResult,
    RaidDefinition, RaidResult, Raider, RaiderId, RaiderState, RaiderTarget,
};
use std::collections::{BTreeMap, BTreeSet};

impl SettlementGame {
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

    pub fn current_raid(&self) -> Option<&RaidDefinition> {
        self.active_raid.as_ref().map(|raid| &raid.definition)
    }

    pub fn last_raid_result(&self) -> Option<&RaidResult> {
        self.last_raid_result.as_ref()
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

    pub(super) fn kill_raider(&mut self, raider_id: RaiderId) {
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
}
