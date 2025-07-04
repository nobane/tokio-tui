// tokio-tui/proc-macro/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Field, Fields, FieldsNamed, Ident, LitBool, LitStr, parse_macro_input,
};

// Helper function to convert snake_case to Title Case
fn snake_to_title_case(input: &str) -> String {
    input
        .split('_')
        .map(|word| {
            if word.is_empty() {
                String::new()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        let first_upper = first.to_uppercase().collect::<String>();
                        let rest: String = chars.collect();
                        format!("{first_upper}{rest}")
                    }
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[proc_macro_derive(TuiEdit, attributes(field))]
pub fn derive_tui_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    match input.data {
        Data::Struct(data_struct) => {
            // Handle structs - implement FormData and SubFormData
            let fields = match data_struct.fields {
                Fields::Named(fields) => fields,
                _ => {
                    return syn::Error::new_spanned(
                        input.ident,
                        "TuiEdit can only be derived for structs with named fields",
                    )
                    .to_compile_error()
                    .into();
                }
            };

            let field_definitions = generate_field_definitions(&fields);
            let to_fields_impl = generate_to_fields_impl(&fields);
            let from_fields_impl = generate_from_fields_impl(&fields);

            // Generate FormData and SubFormData implementations
            let expanded = quote! {
                impl ::tokio_tui::FormData for #name {
                    fn field_definitions() -> Vec<::tokio_tui::FieldMeta> {
                        vec![
                            #(#field_definitions),*
                        ]
                    }

                    fn to_fields(&self) -> std::collections::HashMap<String, ::tokio_tui::FormFieldWidget> {
                        let mut fields = std::collections::HashMap::new();
                        #(#to_fields_impl)*
                        fields
                    }

                    fn from_fields(fields: &std::collections::HashMap<String, ::tokio_tui::FormFieldWidget>) -> Self {
                        Self {
                            #(#from_fields_impl),*
                        }
                    }
                }

                // Automatically implement SubFormData for structs
                impl ::tokio_tui::SubFormData for #name {
                    fn to_form_widget(&self) -> ::tokio_tui::FormWidget {
                        ::tokio_tui::FormWidget::new_nested().with_data(self)
                    }

                    fn from_form_widget(form: &::tokio_tui::FormWidget) -> Self {
                        <Self as ::tokio_tui::FormData>::from_fields(form.get_fields())
                    }
                }
            };

            TokenStream::from(expanded)
        }
        Data::Enum(data_enum) => {
            // Handle enums - implement EnumFormValue
            let variants: Vec<&Ident> = data_enum
                .variants
                .iter()
                .map(|variant| &variant.ident)
                .collect();

            let variant_strings: Vec<String> =
                variants.iter().map(|ident| ident.to_string()).collect();

            let expanded = quote! {
                impl ::tokio_tui::EnumFormValue for #name {
                    fn all_options() -> Vec<Self> {
                        vec![
                            #(Self::#variants),*
                        ]
                    }

                    fn to_string(&self) -> String {
                        match self {
                            #(Self::#variants => #variant_strings.to_string()),*
                        }
                    }

                    fn from_string(s: &str) -> Option<Self> {
                        match s {
                            #(#variant_strings => Some(Self::#variants)),*,
                            _ => None,
                        }
                    }
                }
            };

            TokenStream::from(expanded)
        }
        _ => syn::Error::new_spanned(
            input.ident,
            "TuiEdit can only be derived for structs or enums",
        )
        .to_compile_error()
        .into(),
    }
}

fn generate_field_definitions(fields: &FieldsNamed) -> Vec<proc_macro2::TokenStream> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref()?;
            let field_name_str = field_name.to_string();

            let (label, required, help) = parse_field_attr(field, &field_name_str);

            let help_expr = if let Some(help_text) = help {
                quote! { Some(#help_text) }
            } else {
                quote! { None }
            };

            Some(quote! {
                ::tokio_tui::FieldMeta {
                    id: #field_name_str,
                    label: #label,
                    required: #required,
                    help_text: #help_expr
                }
            })
        })
        .collect()
}

fn generate_to_fields_impl(fields: &FieldsNamed) -> Vec<proc_macro2::TokenStream> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref()?;
            let field_name_str = field_name.to_string();

            Some(quote! {
                {
                    let defs = Self::field_definitions();
                    let meta = defs.iter()
                        .find(|m| m.id == #field_name_str)
                        .expect(&format!("Field metadata not found for {}", #field_name_str));

                    let mut field = <_ as ::tokio_tui::FormValue>::to_field_widget(
                        &self.#field_name,
                        meta.label,
                        meta.required
                    );

                    if let Some(help) = meta.help_text {
                        field = field.with_help_text(help);
                    }

                    fields.insert(#field_name_str.to_string(), field);
                }
            })
        })
        .collect()
}

fn generate_from_fields_impl(fields: &FieldsNamed) -> Vec<proc_macro2::TokenStream> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref()?;
            let field_name_str = field_name.to_string();

            Some(quote! {
                #field_name: if let Some(field) = fields.get(#field_name_str) {
                    <_ as ::tokio_tui::FormValue>::from_field_widget(field)
                } else {
                    // Default value if field is missing
                    Default::default()
                }
            })
        })
        .collect()
}

fn parse_field_attr(field: &Field, field_name: &str) -> (String, bool, Option<String>) {
    let mut label = None;
    let mut required = None;
    let mut help = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("field") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            let path = meta.path.get_ident().unwrap().to_string();

            if path == "label" {
                let value: LitStr = meta.value()?.parse()?;
                label = Some(value.value());
            } else if path == "required" {
                let value: LitBool = meta.value()?.parse()?;
                required = Some(value.value());
            } else if path == "help" {
                let value: LitStr = meta.value()?.parse()?;
                help = Some(value.value());
            }

            Ok(())
        });
    }

    // Default label: convert field_name from snake_case to Title Case
    let final_label = label.unwrap_or_else(|| snake_to_title_case(field_name));

    // Default required: true
    let final_required = required.unwrap_or(true);

    (final_label, final_required, help)
}
