mod generator;
mod spec;
mod validator;

pub use generator::generate_playable_map;
pub use spec::{
    GeneratedSettlementMap, PlayabilityReport, ResourceRegion, ResourceRegionKind,
    SettlementMapSpec,
};
pub use validator::validate_playable_map;
