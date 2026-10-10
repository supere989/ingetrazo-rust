// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Lightweight UI translation (i18n).
//!
//! English is the source language: keys passed to `tr` are English strings.
//! Untranslated strings naturally fall back to English.
//! Embedded catalogs support English ("en"), Spanish ("es"), Portuguese ("pt-BR"),
//! Italian ("it"), Indonesian ("id"), and Simplified Chinese ("zh-CN").

use std::collections::HashMap;
use std::sync::RwLock;

pub const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Español"),
    ("pt-BR", "Português (Brasil)"),
    ("it", "Italiano"),
    ("id", "Bahasa Indonesia"),
    ("zh-CN", "简体中文"),
];

struct I18nCatalog {
    #[allow(dead_code)]
    lang: String,
    catalog: HashMap<String, String>,
}

static CURRENT_CATALOG: RwLock<Option<I18nCatalog>> = RwLock::new(None);
static ACTIVE_LANG: RwLock<String> = RwLock::new(String::new());

fn load_catalog_data(lang: &str) -> Option<HashMap<String, String>> {
    let raw = match lang {
        "es" => include_str!("../../../ingetrazo/i18n/es.json"),
        "pt-BR" => include_str!("../../../ingetrazo/i18n/pt-BR.json"),
        "it" => include_str!("../../../ingetrazo/i18n/it.json"),
        "id" => include_str!("../../../ingetrazo/i18n/id.json"),
        "zh-CN" => include_str!("../../../ingetrazo/i18n/zh-CN.json"),
        _ => return None,
    };
    serde_json::from_str::<HashMap<String, String>>(raw).ok()
}

/// Set active UI language code ("en", "es", "pt-BR", "it", "id", "zh-CN").
pub fn set_language(lang: &str) {
    let mut cur_lang = ACTIVE_LANG.write().unwrap();
    *cur_lang = lang.to_string();

    let mut cat_lock = CURRENT_CATALOG.write().unwrap();
    if lang == "en" || lang.is_empty() {
        *cat_lock = None;
    } else if let Some(map) = load_catalog_data(lang) {
        *cat_lock = Some(I18nCatalog {
            lang: lang.to_string(),
            catalog: map,
        });
    } else {
        *cat_lock = None;
    }
}

/// Get currently active language code.
pub fn current_language() -> String {
    let lang = ACTIVE_LANG.read().unwrap();
    if lang.is_empty() {
        "en".to_string()
    } else {
        lang.clone()
    }
}

/// Translate text into currently active language.
/// If active language is English or string is missing, returns English source text.
pub fn tr(text: &str) -> String {
    let cat_lock = CURRENT_CATALOG.read().unwrap();
    if let Some(ref cat) = *cat_lock {
        if let Some(translated) = cat.catalog.get(text) {
            return translated.clone();
        }
    }
    text.to_string()
}

/// Translate template text and replace placeholder tokens `{key}` with values.
pub fn tr_fmt(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(text);
    for &(key, val) in args {
        let pattern = format!("{{{key}}}");
        out = out.replace(&pattern, val);
    }
    out
}
