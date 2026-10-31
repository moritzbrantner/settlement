use crate::{GridPos, ResourceStack};
use std::collections::{BTreeMap, BTreeSet};

pub type Wall = Obstacle;
pub type ObstacleBuilding = Obstacle;

const ARROW_PROJECTILE_SPEED: f64 = 6.0;
const ROCK_PROJECTILE_SPEED: f64 = 2.5;
const ROCK_DAMAGE_MULTIPLIER: f64 = 0.5;
const ROCK_COOLDOWN_MULTIPLIER: f64 = 2.0;

pub type BuildingId = u32;

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

    pub(crate) fn kind_name(&self) -> &'static str {
        match self.kind {
            BuildingKind::Production(_) => "production",
            BuildingKind::Storage(_) => "storage",
            BuildingKind::Defense(_) => "defense",
            BuildingKind::Obstacle(_) => "obstacle",
            BuildingKind::Ruin(_) => "ruin",
        }
    }
}
