use super::SettlementGame;
use crate::{BlockerHandling, EPSILON, GridPos, PathOptions, PathResult, SettlementMoveProfile};
use farm_nav::{NavDomain, PathRequest, astar};

const MIN_MOVEMENT_COST: f64 = 0.5;
const BLOCKER_PENALTY_COST: f64 = 25.0;

struct SettlementNavDomain<'a> {
    game: &'a SettlementGame,
    nav_version: u64,
}

impl<'a> SettlementNavDomain<'a> {
    fn new(game: &'a SettlementGame) -> Self {
        Self {
            game,
            nav_version: game.nav_version,
        }
    }
}

impl NavDomain for SettlementNavDomain<'_> {
    type Node = GridPos;
    type Profile = SettlementMoveProfile;

    fn neighbors(
        &self,
        node: Self::Node,
        profile: Self::Profile,
        out: &mut Vec<(Self::Node, f32)>,
    ) {
        out.clear();
        for neighbor in self.game.map.neighbors(node).into_iter().flatten() {
            let options = match profile {
                SettlementMoveProfile::Worker => PathOptions::worker(),
                SettlementMoveProfile::Raider => PathOptions::raider(),
                SettlementMoveProfile::RaiderCanBreakBlockers => {
                    PathOptions::raider_with_blockers()
                }
            };
            let Some(cost) = self.game.tile_movement_cost(neighbor, options) else {
                continue;
            };
            out.push((neighbor, cost as f32));
        }
    }

    fn heuristic(&self, from: Self::Node, goal: Self::Node, _: Self::Profile) -> f32 {
        from.manhattan_distance(goal) as f32 * MIN_MOVEMENT_COST as f32
    }

    fn version(&self) -> u64 {
        self.nav_version
    }
}

impl SettlementGame {
    pub fn find_path(&self, start: GridPos, goal: GridPos, options: PathOptions) -> PathResult {
        if !self.map.in_bounds(start) || !self.map.in_bounds(goal) {
            return PathResult::not_found();
        }
        let nav_result = astar(
            &SettlementNavDomain::new(self),
            PathRequest {
                start,
                goal,
                profile: SettlementMoveProfile::from(options),
                allow_partial: false,
            },
        );
        PathResult {
            found: nav_result.found,
            path: nav_result.nodes,
            path_length: nav_result.step_count,
            total_movement_cost: nav_result.total_cost as f64,
        }
    }

    pub fn recalculate_paths_after_map_change(&mut self) {
        self.paths_dirty = true;
        self.nav_version = self.nav_version.wrapping_add(1);
    }

    pub(super) fn advance_unit_route(
        &mut self,
        delta_time: f64,
        fallback_position: GridPos,
        unit_id: u32,
        path_options: PathOptions,
        is_raider: bool,
    ) -> bool {
        let Some(mut route) = (if is_raider {
            self.raider_routes.get(&unit_id).cloned()
        } else {
            self.worker_routes.get(&unit_id).cloned()
        }) else {
            return false;
        };
        let speed = if is_raider {
            self.raiders
                .get(&unit_id)
                .map(|raider| raider.speed)
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let mut remaining_budget = speed * delta_time;
        let mut current_position = if is_raider {
            self.raiders
                .get(&unit_id)
                .map(|raider| raider.position)
                .unwrap_or(fallback_position)
        } else {
            self.workers
                .get(&unit_id)
                .map(|worker| worker.position)
                .unwrap_or(fallback_position)
        };

        while remaining_budget > EPSILON && !route.is_finished() {
            let next_pos = route.path[route.next_index + 1];
            let Some(edge_cost) = self.tile_movement_cost(next_pos, path_options) else {
                return false;
            };
            let remaining_edge_cost = edge_cost - route.progress_on_edge;
            if remaining_budget + EPSILON >= remaining_edge_cost {
                remaining_budget -= remaining_edge_cost;
                route.progress_on_edge = 0.0;
                route.next_index += 1;
                current_position = next_pos;
            } else {
                route.progress_on_edge += remaining_budget;
                remaining_budget = 0.0;
            }
        }

        if is_raider {
            if let Some(raider) = self.raiders.get_mut(&unit_id) {
                raider.position = current_position;
            }
            self.raider_routes.insert(unit_id, route.clone());
        } else if let Some(worker) = self.workers.get_mut(&unit_id) {
            worker.position = current_position;
            self.worker_routes.insert(unit_id, route.clone());
        }

        route.is_finished()
    }

    fn tile_movement_cost(&self, pos: GridPos, options: PathOptions) -> Option<f64> {
        let tile = self.map.tile(pos);
        let base_cost = tile
            .road
            .as_ref()
            .map(|road| road.movement_cost)
            .or_else(|| tile.terrain.movement_cost())?;
        match tile.building {
            Some(building_id) => {
                let building = self.buildings.get(&building_id)?;
                if building.walkable {
                    Some(base_cost)
                } else if matches!(
                    options.blocker_handling,
                    BlockerHandling::IgnoreDestructible
                ) && building.is_destructible_blocker()
                {
                    Some(base_cost + BLOCKER_PENALTY_COST)
                } else {
                    None
                }
            }
            None => Some(base_cost),
        }
    }
}
