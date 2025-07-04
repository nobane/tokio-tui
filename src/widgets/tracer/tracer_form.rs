// tokio-tui/src/widgets/tracer/tracer_form.rs
use serde::Serialize;
use tokio_tui_macro::TuiEdit;
use tracing::Level;

use crate::TuiList;

// Define a wrapper enum for boolean value for forms
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TuiEdit)]
pub enum Inclusion {
    #[default]
    INCLUDE,
    EXCLUDE,
}

impl From<bool> for Inclusion {
    fn from(value: bool) -> Self {
        if value {
            Inclusion::INCLUDE
        } else {
            Inclusion::EXCLUDE
        }
    }
}

impl From<Inclusion> for bool {
    fn from(value: Inclusion) -> Self {
        match value {
            Inclusion::INCLUDE => true,
            Inclusion::EXCLUDE => false,
        }
    }
}

// Trace level form enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default, TuiEdit)]
pub enum TraceLevelForm {
    ERROR,
    WARN,
    INFO,
    #[default]
    DEBUG,
    TRACE,
}

impl From<tokio_tracer::TraceLevel> for TraceLevelForm {
    fn from(level: tokio_tracer::TraceLevel) -> Self {
        match level.0 {
            Level::ERROR => TraceLevelForm::ERROR,
            Level::WARN => TraceLevelForm::WARN,
            Level::INFO => TraceLevelForm::INFO,
            Level::DEBUG => TraceLevelForm::DEBUG,
            Level::TRACE => TraceLevelForm::TRACE,
        }
    }
}

impl From<TraceLevelForm> for tokio_tracer::TraceLevel {
    fn from(form: TraceLevelForm) -> Self {
        match form {
            TraceLevelForm::ERROR => tokio_tracer::TraceLevel(Level::ERROR),
            TraceLevelForm::WARN => tokio_tracer::TraceLevel(Level::WARN),
            TraceLevelForm::INFO => tokio_tracer::TraceLevel(Level::INFO),
            TraceLevelForm::DEBUG => tokio_tracer::TraceLevel(Level::DEBUG),
            TraceLevelForm::TRACE => tokio_tracer::TraceLevel(Level::TRACE),
        }
    }
}

// Trace filter form struct
#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct TraceFilterForm {
    pub level: TraceLevelForm,
    pub include: Inclusion,
    pub module_patterns: Vec<String>,
    pub file_patterns: Vec<String>,
    pub span_patterns: Vec<String>,
    pub target_patterns: Vec<String>,
}

impl From<&tokio_tracer::Matcher> for TraceFilterForm {
    fn from(filter: &tokio_tracer::Matcher) -> Self {
        Self {
            level: filter.level.into(),
            include: filter.include.into(),
            module_patterns: filter.module_patterns.clone(),
            file_patterns: filter.file_patterns.clone(),
            span_patterns: filter.span_patterns.clone(),
            target_patterns: filter.target_patterns.clone(),
        }
    }
}

impl From<TraceFilterForm> for tokio_tracer::Matcher {
    fn from(form: TraceFilterForm) -> Self {
        Self {
            level: form.level.into(),
            include: form.include.into(),
            module_patterns: form.module_patterns.clone(),
            file_patterns: form.file_patterns.clone(),
            span_patterns: form.span_patterns.clone(),
            target_patterns: form.target_patterns.clone(),
        }
    }
}

// Subscriber config form struct
#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct SubscriberConfigForm {
    pub name: String,
    pub filters: TuiList<TraceFilterForm>,
}

impl From<tokio_tracer::TracerTab> for SubscriberConfigForm {
    fn from(config: tokio_tracer::TracerTab) -> Self {
        Self {
            name: config.name,
            filters: TuiList(
                config
                    .matcher_set
                    .iter_matchers()
                    .into_iter()
                    .map(|f| f.into())
                    .collect(),
            ),
        }
    }
}

impl From<SubscriberConfigForm> for tokio_tracer::TracerTab {
    fn from(form: SubscriberConfigForm) -> Self {
        let mut filter_set = tokio_tracer::MatcherSet::empty();
        for filter in form.filters.0 {
            filter_set.add_matcher(filter.into());
        }
        Self {
            name: form.name,
            matcher_set: filter_set,
        }
    }
}

// Tracer config form struct
#[derive(Debug, Clone, Default, Serialize, TuiEdit)]
pub struct TracerConfigForm {
    pub subscribers: TuiList<SubscriberConfigForm>,
}

impl From<tokio_tracer::TracerConfig> for TracerConfigForm {
    fn from(config: tokio_tracer::TracerConfig) -> Self {
        Self {
            subscribers: TuiList(config.tabs.into_iter().map(|s| s.into()).collect()),
        }
    }
}

impl From<TracerConfigForm> for tokio_tracer::TracerConfig {
    fn from(form: TracerConfigForm) -> Self {
        Self {
            tabs: form.subscribers.0.into_iter().map(|s| s.into()).collect(),
        }
    }
}
