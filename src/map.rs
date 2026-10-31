use crate::{BuildingId, LaneDefinition};
use std::collections::BTreeMap;

pub type TileMap = GameMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn manhattan_distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceStack {
    pub resource: String,
    pub amount: f64,
}

impl ResourceStack {
    pub fn new(resource: impl Into<String>, amount: f64) -> Self {
        Self {
            resource: resource.into(),
            amount,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TerrainType {
    Ground,
    Rough,
    Fertile,
    Forest,
    Water,
    Blocked,
}

impl TerrainType {
    pub(crate) fn movement_cost(self) -> Option<f64> {
        match self {
            Self::Ground => Some(1.0),
            Self::Rough => Some(2.0),
            Self::Fertile => Some(1.0),
            Self::Forest => Some(2.5),
            Self::Water => None,
            Self::Blocked => None,
        }
    }

    pub fn is_walkable(self) -> bool {
        self.movement_cost().is_some()
    }

    pub fn is_buildable(self) -> bool {
        matches!(self, Self::Ground | Self::Rough | Self::Fertile)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Road {
    pub movement_cost: f64,
}

impl Default for Road {
    fn default() -> Self {
        Self { movement_cost: 0.5 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub terrain: TerrainType,
    pub road: Option<Road>,
    pub building: Option<BuildingId>,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            terrain: TerrainType::Ground,
            road: None,
            building: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameMap {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    pub lanes: BTreeMap<String, LaneDefinition>,
}

impl GameMap {
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0, "map dimensions must be positive");
        Self {
            width,
            height,
            tiles: vec![Tile::default(); (width * height) as usize],
            lanes: BTreeMap::new(),
        }
    }

    pub fn in_bounds(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    pub fn neighbors(&self, pos: GridPos) -> [Option<GridPos>; 4] {
        [
            self.in_bounds(GridPos::new(pos.x + 1, pos.y))
                .then_some(GridPos::new(pos.x + 1, pos.y)),
            self.in_bounds(GridPos::new(pos.x - 1, pos.y))
                .then_some(GridPos::new(pos.x - 1, pos.y)),
            self.in_bounds(GridPos::new(pos.x, pos.y + 1))
                .then_some(GridPos::new(pos.x, pos.y + 1)),
            self.in_bounds(GridPos::new(pos.x, pos.y - 1))
                .then_some(GridPos::new(pos.x, pos.y - 1)),
        ]
    }

    pub fn tile(&self, pos: GridPos) -> &Tile {
        &self.tiles[self.index(pos)]
    }

    pub fn tile_mut(&mut self, pos: GridPos) -> &mut Tile {
        let index = self.index(pos);
        &mut self.tiles[index]
    }

    pub fn set_terrain(&mut self, pos: GridPos, terrain: TerrainType) {
        self.tile_mut(pos).terrain = terrain;
    }

    pub fn place_road(&mut self, pos: GridPos, road: Road) {
        self.tile_mut(pos).road = Some(road);
    }

    pub fn remove_road(&mut self, pos: GridPos) {
        self.tile_mut(pos).road = None;
    }

    pub fn add_lane(&mut self, lane: LaneDefinition) {
        self.lanes.insert(lane.id.clone(), lane);
    }

    fn index(&self, pos: GridPos) -> usize {
        assert!(self.in_bounds(pos), "position out of bounds");
        (pos.y * self.width + pos.x) as usize
    }
}
