use crate::model::GhostReason;

pub fn classify_ghost(
    declared_size: u64,
    attributed_size: Option<u64>,
    asset_present: bool,
) -> Option<GhostReason> {
    if !asset_present {
        return Some(GhostReason::MissingAsset);
    }
    match attributed_size {
        None => Some(GhostReason::Unmapped),
        Some(0) if declared_size > 0 => Some(GhostReason::EmptyAttribution),
        _ => None,
    }
}
