// tokio-tui/src/widgets/status/status_update.rs
use super::{CellId, StatusLineId};

// Type-erased update function that operates on Any
pub type UpdateFn = Box<dyn FnOnce(&mut dyn std::any::Any) + Send + Sync>;

pub struct StatusCellUpdate {
    pub line_id: StatusLineId,
    pub cell_id: CellId,
    pub update_fn: UpdateFn,
}

pub enum StatusUpdate {
    CellUpdate(StatusCellUpdate),
    LineVisibility {
        line_id: StatusLineId,
        visible: bool,
    },
}

impl From<StatusCellUpdate> for StatusUpdate {
    fn from(value: StatusCellUpdate) -> Self {
        StatusUpdate::CellUpdate(value)
    }
}

impl From<StatusCellUpdate> for Vec<StatusCellUpdate> {
    fn from(value: StatusCellUpdate) -> Self {
        vec![value]
    }
}

pub trait IntoStatusUpdates {
    fn into_status_updates(self) -> Vec<StatusUpdate>;
}

impl IntoStatusUpdates for Vec<StatusCellUpdate> {
    fn into_status_updates(self) -> Vec<StatusUpdate> {
        self.into_iter().map(|i| i.into()).collect()
    }
}

impl IntoStatusUpdates for StatusCellUpdate {
    fn into_status_updates(self) -> Vec<StatusUpdate> {
        vec![self.into()]
    }
}

impl IntoStatusUpdates for StatusUpdate {
    fn into_status_updates(self) -> Vec<StatusUpdate> {
        vec![self]
    }
}

impl IntoStatusUpdates for Vec<StatusUpdate> {
    fn into_status_updates(self) -> Vec<StatusUpdate> {
        self
    }
}

#[macro_export]
macro_rules! batch_updates {
    ($($update:expr),* $(,)?) => {{
        let mut updates = Vec::new();
        $(
            updates.extend(<_ as $crate::IntoStatusUpdates>::into_status_updates($update));
        )*
        updates
    }};
}
