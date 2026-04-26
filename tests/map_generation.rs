use settlement_game::{
    GridPos, PathOptions, SettlementGame, SettlementMapSpec, generate_playable_map,
    validate_playable_map,
};

fn test_spec(seed: u64) -> SettlementMapSpec {
    SettlementMapSpec {
        seed,
        width: 16,
        height: 16,
        starting_clear_radius: 3,
        min_buildable_tiles: 40,
        min_fertile_tiles: 8,
        min_forest_tiles: 8,
        min_water_tiles: 6,
        max_generation_attempts: 16,
    }
}

#[test]
fn same_seed_generates_the_same_map() {
    let spec = test_spec(7);
    let first = generate_playable_map(&spec);
    let second = generate_playable_map(&spec);

    assert_eq!(first, second);
}

#[test]
fn different_seeds_generate_different_maps() {
    let first = generate_playable_map(&test_spec(7));
    let second = generate_playable_map(&test_spec(8));

    assert_ne!(first.terrain, second.terrain);
}

#[test]
fn generated_maps_meet_minimum_tile_counts_and_are_playable() {
    let spec = test_spec(11);
    let map = generate_playable_map(&spec);
    let report = validate_playable_map(&map, &spec);

    assert!(report.valid, "{:?}", report.failures);
    assert!(report.connected_buildable_tiles >= spec.min_buildable_tiles);
    assert!(report.fertile_tiles >= spec.min_fertile_tiles);
    assert!(report.forest_tiles >= spec.min_forest_tiles);
    assert!(report.water_tiles >= spec.min_water_tiles);
}

#[test]
fn generated_maps_keep_required_resource_regions_reachable() {
    let spec = test_spec(19);
    let map = generate_playable_map(&spec);
    let report = validate_playable_map(&map, &spec);

    assert!(report.valid, "{:?}", report.failures);
    assert!(report.reachable_fertile_regions > 0);
    assert!(report.reachable_forest_regions > 0);
    assert!(report.reachable_water_access_regions > 0);
}

#[test]
fn workers_do_not_route_through_non_walkable_buildings_on_generated_maps() {
    let spec = test_spec(23);
    let map = generate_playable_map(&spec);
    let mut game = SettlementGame::new(spec.width, spec.height);
    map.apply_to_game(&mut game);

    let spawn = map.recommended_spawn;
    let blocker = GridPos::new(spawn.x + 1, spawn.y);
    let goal = GridPos::new(spawn.x + 3, spawn.y);
    let initial = game.find_path(spawn, goal, PathOptions::worker());
    assert!(initial.found);
    game.add_obstacle(blocker, 20.0);

    let rerouted = game.find_path(spawn, goal, PathOptions::worker());
    assert!(rerouted.found);
    assert!(!rerouted.path.contains(&blocker));
}
