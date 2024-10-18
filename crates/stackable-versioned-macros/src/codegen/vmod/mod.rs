use darling::FromAttributes;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, Item, ItemMod, Result};

use crate::{
    attrs::common::{ModuleAttributes, NestedContainerAttributes},
    codegen::{common::VersionDefinition, venum::VersionedEnum, vstruct::VersionedStruct},
};

pub(crate) struct VersionedModule {
    module: ItemMod,
    // TODO (@Techassi): This will change
    attributes: ModuleAttributes,
}

pub(crate) enum ModuleItem {
    Struct(VersionedStruct),
    Enum(VersionedEnum),
}

impl VersionedModule {
    pub(crate) fn new(module: ItemMod, attributes: ModuleAttributes) -> Result<Self> {
        let versions: Vec<VersionDefinition> = (&attributes).into();

        let Some((_, items)) = &module.content else {
            return Err(Error::new(module.span(), "module cannot be empty"));
        };

        // let mut versioned_items = Vec::new();

        for item in items {
            match item {
                Item::Enum(item_enum) => {
                    let module_item_attributes =
                        NestedContainerAttributes::from_attributes(&item_enum.attrs)?;
                    // let versioned_enum = VersionedEnum::new(module_item_attributes)
                    // versioned_item.push(ModuleItem(versioned_enum))
                }
                Item::Struct(item_struct) => todo!(),
                _ => todo!(),
            }
        }

        Ok(VersionedModule { module, attributes })
    }

    pub(crate) fn generate_tokens(&self) -> TokenStream {
        quote! {}
    }
}
