use settlement_game::{
    Building, BuildingKind, DefenseBuilding, DefenseWeapon, GridPos, LaneDefinition, PathOptions,
    RaidDefinition, RaiderState, RaiderTemplate, SettlementGame, SimulationConfig, WaveDefinition,
};

const EPSILON: f64 = 0.000_1;

fn configured_game(width: i32, height: i32) -> SettlementGame {
    let mut game = SettlementGame::new(width, height).with_config(SimulationConfig {
        day_length: 1_000.0,
        stored_value_weight: 1.0,
        path_cost_weight: 1.0,
        simulation_step: 0.25,
    });
    game.map.lanes.clear();
    game.add_lane(LaneDefinition::new(
        "north",
        GridPos::new(width / 2, 0),
        GridPos::new(width / 2, 0),
    ));
    game
}

fn single_wave_raid(template: RaiderTemplate) -> RaidDefinition {
    RaidDefinition {
        name: "test_raid".into(),
        waves: vec![WaveDefinition {
            lane_id: "north".into(),
            start_delay: 0.0,
            count: 1,
            raider_type: template,
            name: Some("wave_1".into()),
        }],
    }
}

fn tick_raid_until<F>(game: &mut SettlementGame, mut predicate: F, max_steps: usize)
where
    F: FnMut(&SettlementGame) -> bool,
{
    for _ in 0..max_steps {
        if predicate(game) {
            return;
        }
        let _ = game.tick_raid(0.25);
    }
    panic!("predicate not reached within {max_steps} raid steps");
}

fn test_raider(max_hit_points: f64, speed: f64) -> RaiderTemplate {
    RaiderTemplate {
        name: format!("test_raider_{max_hit_points}_{speed}"),
        max_hit_points,
        speed,
        attack_damage_per_second: 6.0,
        carry_capacity: 10.0,
        looting_rate: 2.0,
        drop_ratio_on_death: 0.75,
    }
}

#[test]
fn production_efficiency_close_storage_beats_far_storage() {
    let mut close = configured_game(12, 7);
    close.add_production_building(GridPos::new(1, 3), "food", 1.0, 20.0);
    close.add_storage_building(GridPos::new(3, 3), 200.0);

    let mut far = configured_game(12, 7);
    far.add_production_building(GridPos::new(1, 3), "food", 1.0, 20.0);
    far.add_storage_building(GridPos::new(10, 3), 200.0);

    let close_result = close.tick_economy(120.0);
    let far_result = far.tick_economy(120.0);

    let close_delivered = close_result
        .resources_delivered_to_storage
        .get("food")
        .copied()
        .unwrap_or(0.0);
    let far_delivered = far_result
        .resources_delivered_to_storage
        .get("food")
        .copied()
        .unwrap_or(0.0);

    assert!(close_delivered > far_delivered + 10.0);
}

#[test]
fn maze_cost_reduces_effective_throughput() {
    let mut open = configured_game(9, 7);
    open.add_production_building(GridPos::new(1, 3), "grain", 1.0, 20.0);
    open.add_storage_building(GridPos::new(7, 3), 200.0);

    let mut maze = configured_game(9, 7);
    maze.add_production_building(GridPos::new(1, 3), "grain", 1.0, 20.0);
    maze.add_storage_building(GridPos::new(7, 3), 200.0);
    for pos in [
        GridPos::new(3, 1),
        GridPos::new(3, 2),
        GridPos::new(3, 3),
        GridPos::new(3, 4),
        GridPos::new(3, 5),
        GridPos::new(5, 1),
        GridPos::new(5, 2),
        GridPos::new(5, 4),
        GridPos::new(5, 5),
    ] {
        maze.add_obstacle(pos, 30.0);
    }

    let open_result = open.tick_economy(120.0);
    let maze_result = maze.tick_economy(120.0);

    let open_delivered = open_result
        .resources_delivered_to_storage
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let maze_delivered = maze_result
        .resources_delivered_to_storage
        .get("grain")
        .copied()
        .unwrap_or(0.0);

    assert!(open_delivered > maze_delivered + 5.0);
    assert!(
        open_result.average_worker_delivery_path_cost.unwrap()
            < maze_result.average_worker_delivery_path_cost.unwrap()
    );
}

#[test]
fn raiders_path_from_fixed_lane_to_storage() {
    let mut game = configured_game(7, 7);
    let storage_id = game.add_storage_building(GridPos::new(3, 5), 100.0);
    game.add_storage_resource(storage_id, "grain", 30.0);
    game.start_raid(single_wave_raid(RaiderTemplate::normal()));

    let _ = game.tick_raid(0.5);
    let raider_id = game.raiders.keys().copied().next().expect("raider spawned");
    let target = game
        .find_best_storage_target_for_raider(raider_id)
        .expect("storage target");

    assert_eq!(target.storage_id, storage_id);
    assert!(target.path.found);
    assert_eq!(
        game.raiders.get(&raider_id).unwrap().position,
        GridPos::new(3, 0)
    );

    let _ = game.tick_raid(1.0);
    let moved_raider = game.raiders.get(&raider_id).unwrap();
    assert!(moved_raider.position.y >= 1);
}

#[test]
fn raiders_loot_storage_over_time() {
    let mut game = configured_game(7, 7);
    let storage_id = game.add_storage_building(GridPos::new(3, 3), 100.0);
    game.add_storage_resource(storage_id, "grain", 10.0);
    game.start_raid(single_wave_raid(RaiderTemplate::small_fast()));

    tick_raid_until(
        &mut game,
        |game| {
            game.raiders
                .values()
                .any(|raider| matches!(raider.state, RaiderState::Looting))
        },
        40,
    );

    let before = game
        .buildings
        .get(&storage_id)
        .and_then(Building::storage_data)
        .unwrap()
        .stored
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let _ = game.tick_raid(1.0);
    let after = game
        .buildings
        .get(&storage_id)
        .and_then(Building::storage_data)
        .unwrap()
        .stored
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let raider = game.raiders.values().next().unwrap();

    assert!(after < before - EPSILON);
    assert!(raider.total_carried() > 0.0);
    assert!(raider.total_carried() < raider.carry_capacity);
}

#[test]
fn raiders_retreat_after_looting() {
    let mut game = configured_game(7, 7);
    let storage_id = game.add_storage_building(GridPos::new(3, 3), 100.0);
    game.add_storage_resource(storage_id, "grain", 4.0);
    game.start_raid(single_wave_raid(RaiderTemplate::small_fast()));

    tick_raid_until(
        &mut game,
        |game| {
            game.raiders.values().any(|raider| {
                matches!(raider.state, RaiderState::Retreating | RaiderState::Escaped)
            })
        },
        80,
    );

    assert!(game.raiders.values().any(|raider| {
        matches!(raider.state, RaiderState::Retreating | RaiderState::Escaped)
            && raider.total_carried() > 0.0
    }));
}

#[test]
fn killing_raider_recovers_some_stolen_resources() {
    let mut game = configured_game(7, 7);
    game.add_defense_building(GridPos::new(2, 4), 3.0, 1, 0.5);
    let storage_id = game.add_storage_building(GridPos::new(3, 4), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);

    let raider = RaiderTemplate {
        name: "fragile_looter".into(),
        max_hit_points: 14.0,
        speed: 1.0,
        attack_damage_per_second: 6.0,
        carry_capacity: 8.0,
        looting_rate: 2.0,
        drop_ratio_on_death: 0.75,
    };
    game.start_raid(single_wave_raid(raider));

    tick_raid_until(
        &mut game,
        |game| game.get_debug_metrics().raiders_killed >= 1,
        200,
    );

    let recovered = game.recovery_pool.get("grain").copied().unwrap_or(0.0);
    assert!(recovered > 0.0);
}

#[test]
fn arrows_track_raiders_after_they_move() {
    let mut game = configured_game(7, 7);
    game.add_building(Building::defense(
        GridPos::new(3, 2),
        DefenseBuilding::new(4.0, 2, 10.0).with_weapons(vec![DefenseWeapon::arrow(4.0, 2, 10.0)]),
    ));
    let storage_id = game.add_storage_building(GridPos::new(3, 6), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);
    game.start_raid(single_wave_raid(test_raider(20.0, 6.0)));

    let _ = game.tick_raid(0.25);
    let raider_id = game.raiders.keys().copied().next().expect("raider spawned");
    let initial_hit_points = game.raiders.get(&raider_id).unwrap().hit_points;

    let _ = game.tick_raid(0.25);
    let moved_position = game.raiders.get(&raider_id).unwrap().position;
    assert_ne!(moved_position, GridPos::new(3, 0));

    let _ = game.tick_raid(0.25);
    let raider = game.raiders.get(&raider_id).unwrap();
    assert!(raider.hit_points < initial_hit_points - EPSILON);
}

#[test]
fn rocks_do_not_track_raiders_that_leave_the_target_tile() {
    let mut game = configured_game(7, 7);
    game.add_building(Building::defense(
        GridPos::new(3, 2),
        DefenseBuilding::new(5.0, 2, 10.0).with_weapons(vec![DefenseWeapon::rock(5.0, 2, 10.0)]),
    ));
    let storage_id = game.add_storage_building(GridPos::new(3, 6), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);
    game.start_raid(single_wave_raid(test_raider(20.0, 6.0)));

    let _ = game.tick_raid(0.25);
    let raider_id = game.raiders.keys().copied().next().expect("raider spawned");
    let initial_hit_points = game.raiders.get(&raider_id).unwrap().hit_points;

    let _ = game.tick_raid(1.0);
    let raider = game.raiders.get(&raider_id).unwrap();
    assert!(raider.position.y > 0);
    assert!((raider.hit_points - initial_hit_points).abs() < EPSILON);
}

#[test]
fn towers_only_use_arrows_outside_rock_range() {
    let mut game = configured_game(7, 7);
    game.add_defense_building(GridPos::new(3, 3), 4.0, 3, 10.0);
    let storage_id = game.add_storage_building(GridPos::new(3, 6), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);
    game.start_raid(single_wave_raid(test_raider(20.0, 1.0)));

    let _ = game.tick_raid(0.25);
    let raider_id = game.raiders.keys().copied().next().expect("raider spawned");
    let initial_hit_points = game.raiders.get(&raider_id).unwrap().hit_points;

    let _ = game.tick_raid(1.0);
    let raider = game.raiders.get(&raider_id).unwrap();
    assert!(raider.hit_points < initial_hit_points - EPSILON);
    assert!(
        raider.hit_points > initial_hit_points - 6.0,
        "rocks should stay out of range when only arrows can reach",
    );
}

#[test]
fn blocked_storage_forces_raiders_to_destroy_blockers() {
    let mut game = configured_game(7, 7);
    let storage_id = game.add_storage_building(GridPos::new(3, 3), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);
    let north_wall = game.add_obstacle(GridPos::new(3, 2), 4.0);
    let west_wall = game.add_obstacle(GridPos::new(2, 3), 20.0);
    let east_wall = game.add_obstacle(GridPos::new(4, 3), 20.0);
    let south_wall = game.add_obstacle(GridPos::new(3, 4), 20.0);

    game.start_raid(single_wave_raid(RaiderTemplate::normal()));
    tick_raid_until(
        &mut game,
        |game| {
            [north_wall, west_wall, east_wall, south_wall]
                .into_iter()
                .any(|wall_id| {
                    game.buildings
                        .get(&wall_id)
                        .is_some_and(|building| matches!(building.kind, BuildingKind::Ruin(_)))
                })
        },
        80,
    );

    assert!(
        [north_wall, west_wall, east_wall, south_wall]
            .into_iter()
            .any(|wall_id| matches!(
                game.buildings.get(&wall_id).unwrap().kind,
                BuildingKind::Ruin(_)
            ))
    );
    assert!(game.get_debug_metrics().blockers_destroyed >= 1);
}

#[test]
fn raiders_choose_between_storages_by_value_and_path_cost() {
    let mut game = configured_game(9, 7);
    let close_storage = game.add_storage_building(GridPos::new(4, 2), 100.0);
    let rich_storage = game.add_storage_building(GridPos::new(4, 5), 100.0);
    game.add_storage_resource(close_storage, "grain", 5.0);
    game.add_storage_resource(rich_storage, "grain", 40.0);
    game.start_raid(single_wave_raid(RaiderTemplate::normal()));

    let _ = game.tick_raid(0.25);
    let raider_id = game.raiders.keys().copied().next().unwrap();
    let target = game
        .find_best_storage_target_for_raider(raider_id)
        .expect("best target");

    assert_eq!(target.storage_id, rich_storage);
}

#[test]
fn nightly_raid_supports_multiple_waves_with_delays() {
    let mut game = configured_game(7, 7);
    let storage_id = game.add_storage_building(GridPos::new(3, 5), 100.0);
    game.add_storage_resource(storage_id, "grain", 100.0);

    game.start_raid(RaidDefinition {
        name: "multi_wave".into(),
        waves: vec![
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 0.0,
                count: 1,
                raider_type: RaiderTemplate::small_fast(),
                name: Some("wave_a".into()),
            },
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 2.0,
                count: 1,
                raider_type: RaiderTemplate::normal(),
                name: Some("wave_b".into()),
            },
            WaveDefinition {
                lane_id: "north".into(),
                start_delay: 4.0,
                count: 1,
                raider_type: RaiderTemplate::bulky(),
                name: Some("wave_c".into()),
            },
        ],
    });

    let _ = game.tick_raid(0.25);
    assert_eq!(game.raiders.len(), 1);

    let _ = game.tick_raid(2.0);
    assert!(game.raiders.len() >= 2);

    let _ = game.tick_raid(2.25);
    assert!(game.raiders.len() >= 3);
}

#[test]
fn failed_defense_is_a_partial_setback_not_a_reset() {
    let mut game = configured_game(7, 7);
    let production_id = game.add_production_building(GridPos::new(1, 5), "food", 1.0, 20.0);
    let storage_id = game.add_storage_building(GridPos::new(3, 3), 100.0);
    game.add_storage_resource(storage_id, "grain", 20.0);
    let wall_id = game.add_obstacle(GridPos::new(3, 2), 4.0);
    game.add_obstacle(GridPos::new(2, 3), 20.0);
    game.add_obstacle(GridPos::new(4, 3), 20.0);
    game.add_obstacle(GridPos::new(3, 4), 20.0);

    game.start_raid(single_wave_raid(RaiderTemplate::normal()));
    tick_raid_until(&mut game, |game| game.last_raid_result().is_some(), 240);

    let result = game.last_raid_result().unwrap();
    assert!(
        result.destroyed_buildings.contains(&wall_id)
            || result.stolen_resources.get("grain").copied().unwrap_or(0.0) > 0.0
    );
    assert!(!game.buildings.get(&production_id).unwrap().is_destroyed());
    let remaining = game
        .buildings
        .get(&storage_id)
        .and_then(Building::storage_data)
        .unwrap()
        .stored
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    assert!(remaining < 20.0);
}

#[test]
fn pathfinding_reports_found_length_cost_and_next_step() {
    let mut game = configured_game(5, 5);
    game.place_road(GridPos::new(1, 0));
    game.place_road(GridPos::new(2, 0));

    let result = game.find_path(
        GridPos::new(0, 0),
        GridPos::new(2, 0),
        PathOptions::worker(),
    );

    assert!(result.found);
    assert_eq!(result.path_length, 2);
    assert_eq!(result.next_step(), Some(GridPos::new(1, 0)));
    assert!(result.total_movement_cost < 2.0);
}
