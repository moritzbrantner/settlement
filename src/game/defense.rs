use super::SettlementGame;
use crate::{
    Building, BuildingId, BuildingKind, DefenseProjectileKind, DefenseTargetPriority,
    DefenseWeapon, EPSILON, GridPos, Raider, RaiderId, RaiderState,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DefenseProjectile {
    kind: DefenseProjectileKind,
    source_id: BuildingId,
    target_raider_id: RaiderId,
    target_position: GridPos,
    damage: f64,
    remaining_time: f64,
    tracking: bool,
}

impl SettlementGame {
    pub(super) fn step_defenses(&mut self, delta_time: f64) {
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

    pub(super) fn step_defense_projectiles(&mut self, delta_time: f64) {
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
}
