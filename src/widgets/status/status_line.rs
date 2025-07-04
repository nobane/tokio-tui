// tokio-tui/src/widgets/status/status_line.rs
use std::{any::Any, marker::PhantomData};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
};

use super::{StatusCellUpdate, StatusUpdate};

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub struct StatusLineId(pub u64);

pub type CellId = usize;

/// Core trait for all status cells that can be displayed in a status line
pub trait StatusCell: Send + Sync {
    fn new<T: Into<Self>>(args: T) -> Self
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn draw_cell(&mut self, area: Rect, buf: &mut Buffer);
    fn constraint(&self) -> Constraint;
    fn needs_draw(&self) -> bool {
        true
    }
    fn preprocess(&mut self) {
        // Default implementation does nothing
    }
}

/// Base trait for status lines that can be added to the manager
pub trait StatusLine {
    fn status_line_ref(&self) -> StatusLineRef;

    fn show(&self) -> StatusUpdate {
        StatusUpdate::LineVisibility {
            line_id: self.status_line_ref().0,
            visible: true,
        }
    }

    fn hide(&self) -> StatusUpdate {
        StatusUpdate::LineVisibility {
            line_id: self.status_line_ref().0,
            visible: false,
        }
    }
}

/// Trait for components that can be added to a status line
pub trait ToStatusCell {
    fn into_status_component(self) -> Box<dyn StatusCell>;
}

/// Type-safe reference to a component within a status line
pub struct CellRef<T: StatusCell> {
    line_id: StatusLineId,
    index: usize,
    _cell_type: PhantomData<T>,
}

impl<T: StatusCell + 'static> CellRef<T> {
    pub fn new(line_id: StatusLineId, index: usize) -> Self {
        Self {
            line_id,
            index,
            _cell_type: PhantomData,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    /// Create a status update for this component using a closure
    pub fn update_with<F>(&self, f: F) -> StatusCellUpdate
    where
        F: FnOnce(&mut T) + Send + Sync + 'static,
    {
        let update_fn = Box::new(move |any_ref: &mut dyn Any| {
            if let Some(typed_ref) = any_ref.downcast_mut::<T>() {
                f(typed_ref);
            }
        });

        StatusCellUpdate {
            line_id: self.line_id,
            cell_id: self.index,
            update_fn,
        }
    }
}

/// Simple wrapper to hold a line ID
#[derive(Clone, Copy, Debug)]
pub struct StatusLineRef(pub StatusLineId);

impl StatusLineRef {
    pub fn new(id: StatusLineId) -> Self {
        Self(id)
    }
}

impl From<StatusLineId> for StatusLineRef {
    fn from(id: StatusLineId) -> Self {
        StatusLineRef(id)
    }
}

impl StatusLine for StatusLineRef {
    fn status_line_ref(&self) -> StatusLineRef {
        *self
    }
}

#[macro_export]
macro_rules! status_line {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident {
            $(
                $field:ident: $cell_type:ty
            ),* $(,)?
        }
    ) => {
        $(#[$attr])*
        $vis struct $name {
            line_ref: $crate::StatusLineRef,
            $(
                pub $field: $crate::CellRef<$cell_type>,
            )*
        }

        impl $name {
            pub fn new(manager: &mut $crate::StatusWidget) -> Self {
                let mut builder = manager.new_builder();

                $(
                    let $field = builder.add(<$cell_type>::default());
                )*

                let line_ref = builder.build(manager);

                Self {
                    line_ref,
                    $(
                        $field,
                    )*
                }
            }

            pub fn with_components(manager: &mut $crate::StatusWidget, $(
                $field: $cell_type,
            )*) -> Self {
                let mut builder = manager.new_builder();

                $(
                    let $field = builder.add($field);
                )*

                let line_ref = builder.build(manager);

                Self {
                    line_ref,
                    $(
                        $field,
                    )*
                }
            }

            pub fn line_ref(&self) -> $crate::StatusLineRef {
                self.line_ref
            }
        }

        impl $crate::StatusLine for $name {
            fn status_line_ref(&self) -> $crate::StatusLineRef {
                self.line_ref
            }
        }
    };
}
