use std::collections::BTreeMap;

use darling::{FromField, Result, util::IdentString};
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Field, Ident, Path, Type, spanned::Spanned};

use crate::{
    attrs::item::{FieldAttributes, Hint},
    codegen::{
        Direction, VersionDefinition,
        changes::{BTreeMapExt, ChangesetExt},
        item::ItemStatus,
        module::ModuleGenerationContext,
    },
    utils::{ItemIdentExt, ItemIdents},
};

pub struct VersionedField {
    pub original_attributes: Vec<Attribute>,
    pub changes: Option<BTreeMap<Version, ItemStatus>>,
    pub idents: FieldIdents,
    pub hint: Option<Hint>,
    pub nested: bool,
    pub ty: Type,
}

impl VersionedField {
    pub fn new(
        field: Field,
        versions: &[VersionDefinition],
        experimental_conversion_tracking: bool,
    ) -> Result<Self> {
        let field_attributes = FieldAttributes::from_field(&field)?;
        field_attributes.validate_versions(versions)?;
        field_attributes.validate_nested_flag(experimental_conversion_tracking)?;

        let field_span = field.span();

        let ident = field.ident.ok_or_else(|| {
            darling::Error::custom("internal error: field must have an ident")
                .with_span(&field_span)
        })?;
        let idents = FieldIdents::from(ident);

        let changes = field_attributes
            .common
            .into_changeset(&idents, field.ty.clone());
        let nested = field_attributes.nested.is_present();

        Ok(Self {
            original_attributes: field_attributes.attrs,
            hint: field_attributes.hint,
            ty: field.ty,
            changes,
            idents,
            nested,
        })
    }

    pub fn insert_container_versions(&mut self, versions: &[VersionDefinition]) {
        if let Some(changes) = &mut self.changes {
            changes.insert_container_versions(versions, &self.ty);
        }
    }

    /// Generates field definitions for the use inside container (struct) definitions.
    ///
    /// This function needs to take into account multiple conditions:
    ///
    /// - Only emit the field if it exists for the currently generated version.
    /// - Emit field with new name and type if there was a name and/or type change.
    /// - Handle deprecated fields accordingly.
    ///
    /// ### Example
    ///
    /// ```ignore
    /// struct Foo {
    ///     bar: usize, // This function generates one or more of these definitions
    /// }
    /// ```
    pub fn generate_for_container(&self, version: &VersionDefinition) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;

        match &self.changes {
            Some(changes) => {
                // Check if the provided container version is present in the map of actions. If it
                // is, some action occurred in exactly that version and thus code is generated for
                // that field based on the type of action.
                // If not, the provided version has no action attached to it. The code generation
                // then depends on the relation to other versions (with actions).
                let field_type = &self.ty;

                // NOTE (@Techassi): `unwrap_or_else` used instead of `expect`.
                // See: https://rust-lang.github.io/rust-clippy/master/index.html#expect_fun_call
                // We could use expect here, but we would lose the version in the panic message. We need to allow
                // a lint in either case anyway.
                #[allow(clippy::panic)]
                match changes.get(&version.inner).unwrap_or_else(|| {
                    panic!(
                        "internal error: chain must contain container version {}",
                        version.inner
                    )
                }) {
                    ItemStatus::Addition { ident, ty, .. } => Some(quote! {
                        #(#original_attributes)*
                        pub #ident: #ty,
                    }),
                    ItemStatus::Change {
                        to_ident, to_type, ..
                    } => Some(quote! {
                        #(#original_attributes)*
                        pub #to_ident: #to_type,
                    }),
                    ItemStatus::Deprecation {
                        ident: field_ident,
                        note,
                        ..
                    } => {
                        // FIXME (@Techassi): Emitting the deprecated attribute should cary over even
                        // when the item status is 'NoChange'.
                        // TODO (@Techassi): Make the generation of deprecated items customizable.
                        // When a container is used as a K8s CRD, the item must continue to exist,
                        // even when deprecated. For other versioning use-cases, that might not be
                        // the case.
                        let deprecated_attr = if let Some(note) = note {
                            quote! {#[deprecated = #note]}
                        } else {
                            quote! {#[deprecated]}
                        };

                        Some(quote! {
                            #(#original_attributes)*
                            #deprecated_attr
                            pub #field_ident: #field_type,
                        })
                    }
                    ItemStatus::NotPresent => None,
                    ItemStatus::NoChange {
                        previously_deprecated,
                        ident,
                        ty,
                        ..
                    } => {
                        // TODO (@Techassi): Also carry along the deprecation note.
                        let deprecated_attr = previously_deprecated.then(|| quote! {#[deprecated]});

                        Some(quote! {
                            #(#original_attributes)*
                            #deprecated_attr
                            pub #ident: #ty,
                        })
                    }
                }
            }
            None => {
                // If there is no chain of field actions, the field is not versioned and therefore
                // included in all versions.
                let field_ident = &self.idents.original;
                let field_type = &self.ty;

                Some(quote! {
                    #(#original_attributes)*
                    pub #field_ident: #field_type,
                })
            }
        }
    }

    /// Generates field definitions for the use inside `From` impl blocks.
    ///
    /// This function needs to take into account multiple conditions:
    ///
    /// - Only emit the field if it exists for the currently generated version.
    /// - Emit fields which previously didn't exist with the correct initialization function.
    /// - Emit field with new name and type if there was a name and/or type change.
    /// - Handle tracking conversions without data-loss.
    /// - Handle deprecated fields accordingly.
    ///
    /// ### Example
    ///
    /// ```ignore
    /// impl From<v1alpha1::Foo> for v1alpha2::Foo {
    ///     fn from(value: v1alpha1::Foo) -> Self {
    ///         Self {
    ///             bar: value.bar, // This function generates one or more of these definitions
    ///         }
    ///     }
    /// }
    /// ```
    pub fn generate_for_from_impl(
        &self,
        direction: Direction,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        from_struct_ident: &IdentString,
    ) -> Option<TokenStream> {
        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    // If both this status and the next one is NotPresent, which means a field was
                    // introduced after a bunch of versions, we don't need to generate any code for
                    // the From impl.
                    (ItemStatus::NotPresent, ItemStatus::NotPresent) => None,
                    (
                        _,
                        ItemStatus::Addition {
                            ident, default_fn, ..
                        },
                    ) => match direction {
                        Direction::Upgrade => Some(quote! { #ident: #default_fn(), }),
                        Direction::Downgrade => None,
                    },
                    (
                        _,
                        ItemStatus::Change {
                            downgrade_with,
                            upgrade_with,
                            from_ident,
                            to_ident,
                            ..
                        },
                    ) => match direction {
                        Direction::Upgrade => Some(self.generate_from_impl_field(
                            to_ident,
                            from_struct_ident,
                            from_ident,
                            upgrade_with.as_ref(),
                        )),
                        Direction::Downgrade => Some(self.generate_from_impl_field(
                            from_ident,
                            from_struct_ident,
                            to_ident,
                            downgrade_with.as_ref(),
                        )),
                    },
                    (old, next) => {
                        let next_field_ident = next.get_ident();
                        let old_field_ident = old.get_ident();

                        // NOTE (@Techassi): Do we really need .into() here. I'm currently not sure
                        // why it is there and if it is needed in some edge cases.
                        match direction {
                            Direction::Upgrade => Some(self.generate_from_impl_field(
                                next_field_ident,
                                from_struct_ident,
                                old_field_ident,
                                None,
                            )),
                            Direction::Downgrade => Some(self.generate_from_impl_field(
                                old_field_ident,
                                from_struct_ident,
                                next_field_ident,
                                None,
                            )),
                        }
                    }
                }
            }
            None => {
                let field_ident = &self.idents.original;

                Some(self.generate_from_impl_field(
                    field_ident,
                    from_struct_ident,
                    field_ident,
                    None,
                ))
            }
        }
    }

    /// Generates code needed when a tracked conversion for this field needs to be inserted into the
    /// status.
    pub fn generate_for_status_insertion(
        &self,
        direction: Direction,
        next_version: &VersionDefinition,
        from_struct_ident: &IdentString,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        let changes = self.changes.as_ref()?;

        match direction {
            // This arm is only relevant for removed fields which are currently
            // not supported.
            Direction::Upgrade => None,

            // When we generate code for a downgrade, any changes which need to
            // be tracked need to be inserted into the upgrade section for the
            // next time an upgrade needs to be done.
            Direction::Downgrade => {
                let next_change = changes.get_expect(&next_version.inner);

                let serde_yaml_path = &*mod_gen_ctx.crates.serde_yaml;
                let versioned_path = &*mod_gen_ctx.crates.versioned;

                match next_change {
                    ItemStatus::Addition { ident, .. } => {
                        // TODO (@Techassi): Only do this formatting once, but that requires extensive
                        // changes to the field ident and changeset generation
                        let json_path_ident = ident.json_path_ident();

                        Some(quote! {
                            upgrades.push(#versioned_path::ChangedValue {
                                json_path: #json_path_ident,
                                value: #serde_yaml_path::to_value(&#from_struct_ident.#ident).unwrap(),
                            });
                        })
                    }
                    _ => None,
                }
            }
        }
    }

    /// Generates code needed when a tracked conversion for this field needs to be removed from the
    /// status.
    pub fn generate_for_status_removal(
        &self,
        direction: Direction,
        next_version: &VersionDefinition,
    ) -> Option<TokenStream> {
        // If there are no changes for this field, there is also no need to generate a match arm
        // for applying a tracked value.
        let changes = self.changes.as_ref()?;

        match direction {
            Direction::Upgrade => {
                let next_change = changes.get_expect(&next_version.inner);

                match next_change {
                    // NOTE (@Techassi): We currently only support tracking added fields. As such
                    // we only need to generate code if the next change is "Addition".
                    ItemStatus::Addition { ident, .. } => {
                        let json_path_ident = ident.json_path_ident();

                        Some(quote! {
                            json_path if json_path == #json_path_ident => {
                                spec.#ident = serde_yaml::from_value(value).unwrap();
                            },
                        })
                    }
                    _ => None,
                }
            }
            Direction::Downgrade => None,
        }
    }

    pub fn generate_for_json_path(
        &self,
        next_version: &VersionDefinition,
        mod_gen_ctx: ModuleGenerationContext<'_>,
    ) -> Option<TokenStream> {
        let versioned_path = &*mod_gen_ctx.crates.versioned;

        match (&self.changes, self.nested) {
            // If there are no changes and the field also not marked as nested, there is no need to
            // generate a path variable for that field as no tracked values need to be applied/inserted
            // and the tracking mechanism doesn't need to be forwarded to a sub struct.
            (None, false) => None,

            // If the field is marked as nested, a path variable for that field needs to be generated
            // which is then passed down to the sub struct. There is however no need to determine if
            // the field itself also has changes. This is explicitly handled by the following match
            // arm.
            (_, true) => {
                let field_ident = &self.idents.json_path;
                let child_string = self.idents.original.to_string();

                Some(quote! {
                    let #field_ident = #versioned_path::jthong_path(parent, #child_string);
                })
            }
            (Some(changes), _) => {
                let next_change = changes.get_expect(&next_version.inner);

                match next_change {
                    ItemStatus::Addition { ident, .. } => {
                        let field_ident = ident.json_path_ident();
                        let child_string = ident.to_string();

                        Some(quote! {
                            let #field_ident = #versioned_path::jthong_path(parent, #child_string);
                        })
                    }
                    _ => None,
                }
            }
        }
    }

    /// Generates field definitions to be used inside `From` impl blocks.
    fn generate_from_impl_field(
        &self,
        lhs_field_ident: &IdentString,
        rhs_struct_ident: &IdentString,
        rhs_field_ident: &IdentString,
        custom_conversion_function: Option<&Path>,
    ) -> TokenStream {
        match custom_conversion_function {
            // The user specified a custom conversion function which will be used here instead of the
            // default conversion call which utilizes From impls.
            Some(convert_fn) => quote! {
                #lhs_field_ident: #convert_fn(#rhs_struct_ident.#rhs_field_ident),
            },
            // Default conversion call using From impls.
            None => {
                if self.nested {
                    let json_path_ident = lhs_field_ident.json_path_ident();
                    let func = self.generate_tracking_conversion_function(json_path_ident);

                    quote! {
                        #lhs_field_ident: #rhs_struct_ident.#rhs_field_ident.#func,
                    }
                } else {
                    let func = self.generate_conversion_function();

                    quote! {
                        #lhs_field_ident: #rhs_struct_ident.#rhs_field_ident.#func,
                    }
                }
            }
        }
    }

    /// Generates tracking conversion functions used by field definitions in `From` impl blocks.
    fn generate_tracking_conversion_function(&self, json_path_ident: IdentString) -> TokenStream {
        match &self.hint {
            Some(hint) => match hint {
                Hint::Option => {
                    quote! { map(|v| v.tracking_into(status, &#json_path_ident)) }
                }
                Hint::Vec => {
                    quote! { into_iter().map(|v| v.tracking_into(status, &#json_path_ident)).collect() }
                }
            },
            None => quote! { tracking_into(status, &#json_path_ident) },
        }
    }

    /// Generates conversion functions used by field definitions in `From` impl blocks.
    fn generate_conversion_function(&self) -> TokenStream {
        match &self.hint {
            Some(hint) => match hint {
                Hint::Option => quote! { map(Into::into) },
                Hint::Vec => quote! { into_iter().map(Into::into).collect() },
            },
            None => quote! { into() },
        }
    }
}

/// A collection of field idents used for different purposes.
#[derive(Debug)]
pub struct FieldIdents {
    /// The original ident.
    pub original: IdentString,

    /// The cleaned ident, with the deprecation prefix removed.
    pub cleaned: IdentString,

    /// The cleaned ident used for JSONPath variables.
    pub json_path: IdentString,
}

impl ItemIdents for FieldIdents {
    const DEPRECATION_PREFIX: &str = "deprecated_";

    fn cleaned(&self) -> &IdentString {
        &self.cleaned
    }

    fn original(&self) -> &IdentString {
        &self.original
    }
}

impl From<Ident> for FieldIdents {
    fn from(ident: Ident) -> Self {
        let original = IdentString::new(ident);
        let cleaned = original
            .clone()
            .map(|s| s.trim_start_matches(Self::DEPRECATION_PREFIX).to_owned());

        let json_path = cleaned.json_path_ident();

        Self {
            json_path,
            original,
            cleaned,
        }
    }
}
