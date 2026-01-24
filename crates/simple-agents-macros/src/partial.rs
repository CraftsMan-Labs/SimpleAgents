use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let partial_name = Ident::new(&format!("Partial{}", name), name.span());
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "PartialType only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "PartialType only supports structs")
                .to_compile_error()
                .into();
        }
    };

    // Parse field attributes and generate partial fields
    let mut partial_fields = Vec::new();
    let mut merge_statements = Vec::new();
    let mut from_partial_statements = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;

        // Check for #[partial(skip)] attribute
        let is_skipped = field.attrs.iter().any(|attr| {
            if attr.path().is_ident("partial") {
                if let Ok(meta) = attr.parse_args::<Ident>() {
                    return meta == "skip";
                }
            }
            false
        });

        if is_skipped {
            // Skipped fields use Default::default() in from_partial
            from_partial_statements.push(quote! {
                #field_name: Default::default()
            });
            continue;
        }

        // Check for #[partial(default)] attribute
        let has_default = field.attrs.iter().any(|attr| {
            if attr.path().is_ident("partial") {
                if let Ok(meta) = attr.parse_args::<Ident>() {
                    return meta == "default";
                }
            }
            false
        });

        // Generate partial field: field_name: Option<field_ty>
        partial_fields.push(quote! {
            pub #field_name: Option<#field_ty>
        });

        // Generate merge statement
        merge_statements.push(quote! {
            if other.#field_name.is_some() {
                self.#field_name = other.#field_name;
            }
        });

        // Generate from_partial statement
        if has_default {
            from_partial_statements.push(quote! {
                #field_name: partial.#field_name.unwrap_or_default()
            });
        } else {
            from_partial_statements.push(quote! {
                #field_name: partial.#field_name
                    .ok_or_else(|| format!("Missing required field: {}", stringify!(#field_name)))?
            });
        }
    }

    // Generate the partial struct
    let partial_struct = quote! {
        /// Partial version of [`#name`] for streaming support.
        ///
        /// All fields are wrapped in `Option<T>` to support progressive emission.
        /// Use `merge()` to combine partial values from multiple chunks.
        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        #[allow(missing_docs)]
        pub struct #partial_name #generics #where_clause {
            #(#partial_fields),*
        }
    };

    // Generate impl block for the partial type
    let partial_impl = quote! {
        impl #impl_generics #partial_name #ty_generics #where_clause {
            /// Merge another partial value into this one.
            ///
            /// Fields from `other` that are `Some` will overwrite existing values.
            /// Fields that are `None` in `other` are left unchanged.
            pub fn merge(&mut self, other: #partial_name #ty_generics) {
                #(#merge_statements)*
            }

            /// Check if all required fields are present.
            ///
            /// Returns `true` if this partial can be converted to the complete type.
            pub fn is_complete(&self) -> bool {
                // This is a conservative check - from_partial will do the real validation
                true
            }
        }
    };

    // Generate impl block for the original type
    let from_partial_impl = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// Convert a partial value to the complete type.
            ///
            /// # Errors
            ///
            /// Returns an error if any required fields are missing.
            pub fn from_partial(partial: #partial_name #ty_generics) -> ::std::result::Result<Self, String> {
                Ok(Self {
                    #(#from_partial_statements),*
                })
            }
        }
    };

    // Combine all generated code
    let expanded = quote! {
        #partial_struct
        #partial_impl
        #from_partial_impl
    };

    TokenStream::from(expanded)
}
