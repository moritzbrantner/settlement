use crate::GridPos;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PathActor {
    Worker,
    Raider,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockerHandling {
    Impassable,
    IgnoreDestructible,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PathOptions {
    pub actor: PathActor,
    pub blocker_handling: BlockerHandling,
}

impl PathOptions {
    pub fn worker() -> Self {
        Self {
            actor: PathActor::Worker,
            blocker_handling: BlockerHandling::Impassable,
        }
    }

    pub fn raider() -> Self {
        Self {
            actor: PathActor::Raider,
            blocker_handling: BlockerHandling::Impassable,
        }
    }

    pub fn raider_with_blockers() -> Self {
        Self {
            actor: PathActor::Raider,
            blocker_handling: BlockerHandling::IgnoreDestructible,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SettlementMoveProfile {
    Worker,
    Raider,
    RaiderCanBreakBlockers,
}

impl From<PathOptions> for SettlementMoveProfile {
    fn from(value: PathOptions) -> Self {
        match (value.actor, value.blocker_handling) {
            (PathActor::Worker, _) => Self::Worker,
            (PathActor::Raider, BlockerHandling::Impassable) => Self::Raider,
            (PathActor::Raider, BlockerHandling::IgnoreDestructible) => {
                Self::RaiderCanBreakBlockers
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathResult {
    pub found: bool,
    pub path: Vec<GridPos>,
    pub path_length: usize,
    pub total_movement_cost: f64,
}

impl PathResult {
    pub(crate) fn not_found() -> Self {
        Self {
            found: false,
            path: Vec::new(),
            path_length: 0,
            total_movement_cost: f64::INFINITY,
        }
    }

    pub fn next_step(&self) -> Option<GridPos> {
        self.path.get(1).copied()
    }
}
