// tokio-tui/src/tui/mode_layout.rs
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::{collections::HashMap, fmt::Debug, hash::Hash};

// Represents a split direction in a container
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl From<SplitDirection> for Direction {
    fn from(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Horizontal => Direction::Horizontal,
            SplitDirection::Vertical => Direction::Vertical,
        }
    }
}

// A simple configuration for a layout with direction and constraints
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub direction: SplitDirection,
    pub constraints: Vec<Constraint>,
}

impl LayoutConfig {
    pub fn new(direction: SplitDirection, constraints: Vec<Constraint>) -> Self {
        Self {
            direction,
            constraints,
        }
    }
}

// Mode-specific layout configuration
#[derive(Debug, Clone, Default)]
pub struct ModeLayout<M: Eq + Hash + Clone + Debug> {
    configs: HashMap<M, LayoutConfig>,
}

impl<M: Eq + Hash + Clone + Debug> ModeLayout<M> {
    // Create a new modal layout
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    // Add a mode-specific configuration
    pub fn with_mode(mut self, mode: M, config: LayoutConfig) -> Self {
        self.configs.insert(mode, config);
        self
    }

    // Split an area according to the current mode
    pub fn split(&self, mode: &M, area: Rect) -> std::rc::Rc<[Rect]> {
        if let Some(config) = self.configs.get(mode) {
            Layout::default()
                .direction(config.direction.into())
                .constraints(config.constraints.clone())
                .split(area)
        } else {
            std::rc::Rc::new([])
        }
    }
}

// Create horizontal layout config
#[macro_export]
macro_rules! horizontal {
    [ $($constraint:ident($n:literal)),* $(,)? ] => {
        $crate::LayoutConfig::new(
            $crate::SplitDirection::Horizontal,
            vec![ $($crate::Constraint::$constraint($n)),* ]
        )
    };
}

// Create vertical layout config
#[macro_export]
macro_rules! vertical {
    [ $($constraint:ident($n:literal)),* $(,)? ] => {
        $crate::LayoutConfig::new(
            $crate::SplitDirection::Vertical,
            vec![ $($crate::Constraint::$constraint($n)),* ]
        )
    };
}

// Create a mode layout with configurations for different modes
#[macro_export]
macro_rules! layout {
    [ $($mode:expr => $config:expr),+ $(,)? ] => {{
        let mut layout = $crate::ModeLayout::new();
        $(
            layout = layout.with_mode($mode, $config);
        )+
        layout
    }};
}
