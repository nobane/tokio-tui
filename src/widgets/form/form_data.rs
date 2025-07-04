// tokio-tui/src/widgets/form/form_data.rs
use std::collections::HashMap;
use std::fmt::Debug;

use super::{FormFieldType, FormFieldWidget, FormWidget};

/// Trait representing a field value that can be used in a form
pub trait FormValue: Clone {
    /// Convert the form value to a field widget
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget;

    /// Update this value from a field widget
    fn from_field_widget(field: &FormFieldWidget) -> Self;
}

/// Implementation for String values
impl FormValue for String {
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget {
        FormFieldWidget::text(label, self.clone(), required)
    }

    fn from_field_widget(field: &FormFieldWidget) -> Self {
        match &field.inner {
            FormFieldType::Text(text_field) => text_field.value.clone(),
            _ => String::new(), // Fallback
        }
    }
}

/// Implementation for Vec<String> values (list fields)
impl FormValue for Vec<String> {
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget {
        FormFieldWidget::string_list(label, self.clone(), required)
    }

    fn from_field_widget(field: &FormFieldWidget) -> Self {
        match &field.inner {
            FormFieldType::List(list_field) => list_field.items.clone(),
            _ => Vec::new(), // Fallback
        }
    }
}

/// Trait for enum types that can be used in select fields
pub trait EnumFormValue: Clone + PartialEq + Debug {
    /// Get all possible options of this enum
    fn all_options() -> Vec<Self>;

    /// Convert this enum value to a string
    fn to_string(&self) -> String;

    /// Create an enum value from a string
    fn from_string(s: &str) -> Option<Self>;

    /// Get the index of this option in the all_options list
    fn get_index(&self) -> usize {
        Self::all_options()
            .iter()
            .position(|opt| opt.to_string() == self.to_string())
            .unwrap_or(0)
    }
}

/// Implementation for EnumFormValue types
impl<T: EnumFormValue> FormValue for T {
    fn to_field_widget(&self, label: &str, required: bool) -> FormFieldWidget {
        let options = T::all_options()
            .iter()
            .map(|option| option.to_string())
            .collect::<Vec<_>>();

        FormFieldWidget::select(label, options, self.get_index(), required)
    }

    fn from_field_widget(field: &FormFieldWidget) -> Self {
        match &field.inner {
            FormFieldType::Select(select_field) => {
                if select_field.selected < T::all_options().len() {
                    return T::all_options()[select_field.selected].clone();
                }
                // Fallback to first option
                T::all_options().first().unwrap().clone()
            }
            _ => T::all_options().first().unwrap().clone(), // Fallback
        }
    }
}

/// Field metadata for a form data struct
pub struct FieldMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub help_text: Option<&'static str>,
}

/// Trait for a struct that can be used as form data
pub trait FormData: Default + Sized {
    /// Get the field definitions for this form data
    fn field_definitions() -> Vec<FieldMeta>;

    /// Convert this form data to a map of field widgets
    fn to_fields(&self) -> HashMap<String, FormFieldWidget>;

    /// Create form data from field widgets
    fn from_fields(fields: &HashMap<String, FormFieldWidget>) -> Self;
}

// Add trait for nested forms
pub trait SubFormData: FormData + Clone + Default {
    fn to_form_widget(&self) -> FormWidget;
    fn from_form_widget(form: &FormWidget) -> Self;
}
