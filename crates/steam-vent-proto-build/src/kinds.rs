use crate::{prost_enum_variant, to_upper_camel};
use proc_macro2::{Ident, Span, TokenStream};
use prost_types::FileDescriptorSet;
use quote::quote;
use std::collections::HashMap;

/// Collect the network-message "kinds" from every `E*Msg`/`E*Messages` enum in
/// the descriptor set.
///
/// The matching metadata is derived from the *proto* names (so the heuristics in
/// [`Kind::matches`] are identical to the rust-protobuf era), while the emitted
/// idents use prost's renaming so they line up with the generated enums.
pub fn get_kinds(fds: &FileDescriptorSet) -> Vec<Kind> {
    let mut kinds = fds
        .file
        .iter()
        .flat_map(|file| {
            file.enum_type
                .iter()
                .filter(|e| {
                    let name = e.name();
                    name.starts_with('E') && (name.ends_with("Msg") || name.ends_with("Messages"))
                })
                .flat_map(|e| {
                    let enum_name = e.name().to_string();
                    // prost emits only the *first* variant for any given numeric
                    // value (proto `allow_alias` duplicates are dropped). Resolve
                    // every variant to that canonical ident up-front so the kind
                    // constant we emit always names a variant that exists. The
                    // wire value is identical, so this is purely cosmetic.
                    let mut canonical: HashMap<i32, String> = HashMap::new();
                    for value in &e.value {
                        canonical
                            .entry(value.number())
                            .or_insert_with(|| prost_enum_variant(&enum_name, value.name()));
                    }
                    e.value.iter().map(move |value| {
                        let variant_ident = canonical[&value.number()].clone();
                        Kind::new(&enum_name, value.name(), variant_ident)
                    })
                })
        })
        .collect::<Vec<_>>();

    // Sort kinds with the longest prefix in front so that more specific kinds
    // (e.g. game-coordinator messages) win over the generic `EMsg` when several
    // could match. The secondary keys make the order fully deterministic so the
    // generated code is byte-stable (checked by the `codegen-sync` CI job).
    kinds.sort_by(|a, b| {
        b.enum_prefix
            .len()
            .cmp(&a.enum_prefix.len())
            .then_with(|| a.enum_ident.cmp(&b.enum_ident))
            .then_with(|| a.variant.cmp(&b.variant))
    });
    kinds
}

#[derive(Debug, Clone)]
pub struct Kind {
    /// prost type ident of the enum, e.g. `EMsg`.
    enum_ident: String,
    /// prost ident of the variant, e.g. `KEMsgMulti`.
    variant_ident: String,
    enum_prefix: String,
    variant_prefix: String,
    variant_prefix_alt: String,
    variant_prefix_alt2: String,
    /// the *proto* variant name, e.g. `k_EMsgMulti` (used for matching).
    variant: String,
    is_gc: bool,
    struct_name_prefix_alt_len: usize,
}

impl Kind {
    pub fn new(enum_name: &str, variant_name: &str, variant_ident: String) -> Self {
        let prefix: String = enum_name
            .chars()
            .skip(1)
            .take_while(char::is_ascii_uppercase)
            .collect();
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            prefix[0..prefix.len() - 1].to_string()
        };
        let variant_prefix = format!("k_EMsg{}", prefix);
        let variant_prefix_alt = format!("k_E{}Msg_", prefix);
        let variant_prefix_alt2 = "k_EMsg".to_string();
        let enum_prefix = prefix.to_ascii_lowercase();

        Kind {
            is_gc: variant_prefix.contains("GC"),
            enum_ident: to_upper_camel(enum_name),
            variant_ident,
            enum_prefix,
            variant_prefix,
            variant_prefix_alt,
            variant_prefix_alt2,
            variant: variant_name.to_string(),
            struct_name_prefix_alt_len: prefix.len(),
        }
    }

    pub fn matches(&self, struct_name: &str, file_name: Option<&str>) -> bool {
        let struct_name = struct_name.strip_prefix('C').unwrap_or(struct_name);
        let struct_name = struct_name.strip_prefix("Msg").unwrap_or(struct_name);

        let Some(stripped) = self
            .variant
            .strip_prefix(&self.variant_prefix)
            .or_else(|| self.variant.strip_prefix(&self.variant_prefix_alt))
            .or_else(|| self.variant.strip_prefix(&self.variant_prefix_alt2))
        else {
            return false;
        };
        if let Some(file_name) = file_name {
            if !(file_name.contains(&self.enum_prefix)
                || file_name.replace('_', "").contains(&self.enum_prefix))
            {
                return false;
            }
        }
        struct_name.eq_ignore_ascii_case(stripped)
            || (self.is_gc
                && stripped
                    .strip_prefix("GC")
                    .unwrap_or_default()
                    .eq_ignore_ascii_case(struct_name))
            || struct_name
                .get(self.struct_name_prefix_alt_len..)
                .unwrap_or_default()
                .eq_ignore_ascii_case(stripped)
    }

    /// `EMsg::KEMsgMulti` — bare path, valid because the trait impls are emitted
    /// into the same flat module as the generated enums.
    pub fn ident(&self) -> TokenStream {
        let enum_ident = Ident::new(&self.enum_ident, Span::call_site());
        let variant_ident = Ident::new(&self.variant_ident, Span::call_site());
        quote!(#enum_ident::#variant_ident)
    }

    /// `EMsg` — bare path (see [`Kind::ident`]).
    pub fn enum_ident(&self) -> TokenStream {
        let enum_ident = Ident::new(&self.enum_ident, Span::call_site());
        quote!(#enum_ident)
    }

    pub fn enum_ident_str(&self) -> &str {
        &self.enum_ident
    }

    pub fn variant_ident_str(&self) -> &str {
        &self.variant_ident
    }

    pub fn variant_proto(&self) -> &str {
        &self.variant
    }
}

#[cfg(test)]
fn kind(enum_name: &str, variant_name: &str) -> Kind {
    Kind::new(
        enum_name,
        variant_name,
        prost_enum_variant(enum_name, variant_name),
    )
}

#[test]
fn test_find_kind() {
    assert!(
        kind("EMsg", "k_EMsgClientSiteLicenseCheckout").matches(
            "CMsgClientSiteLicenseCheckout",
            Some("steammessages_sitelicenseclient")
        )
    );
    assert!(
        kind("EGCItemMsg", "k_EMsgGCApplyAutograph")
            .matches("CMsgApplyAutograph", Some("econ_gcmessages"))
    );

    assert!(kind("EDOTAGCMsg", "k_EMsgGCLobbyList").matches(
        "CMsgLobbyList",
        Some("dota_gcmessages_client_match_management")
    ));
}

#[test]
fn test_prost_variant_naming() {
    // Pins prost's `to_upper_camel` + `strip_enum_prefix` behaviour for the
    // Steam `k_`-prefixed variants: the `k_` keeps the variant from ever sharing
    // the enum's prefix, so nothing is stripped.
    let multi = kind("EMsg", "k_EMsgMulti");
    assert_eq!(multi.enum_ident_str(), "EMsg");
    assert_eq!(multi.variant_ident_str(), "KEMsgMulti");

    let persistence = kind("ESessionPersistence", "k_ESessionPersistence_Persistent");
    assert_eq!(persistence.enum_ident_str(), "ESessionPersistence");
    assert_eq!(
        persistence.variant_ident_str(),
        "KESessionPersistencePersistent"
    );
}
