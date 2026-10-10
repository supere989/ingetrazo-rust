// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use ingetrazo_core::i18n::{current_language, set_language, tr, tr_fmt};

#[test]
fn test_i18n_all_languages_and_fallbacks() {
    // English default
    set_language("en");
    assert_eq!(current_language(), "en");
    assert_eq!(tr("File"), "File");
    assert_eq!(tr("Save"), "Save");
    assert_eq!(tr("Layers"), "Layers");
    assert_eq!(tr_fmt("Sheet {n}", &[("n", "1")]), "Sheet 1");
    assert_eq!(
        tr_fmt("{n} entities selected", &[("n", "5")]),
        "5 entities selected"
    );

    // Spanish translation
    set_language("es");
    assert_eq!(current_language(), "es");
    assert_eq!(tr("File"), "Archivo");
    assert_eq!(tr("Save"), "Guardar");
    assert_eq!(tr("Layers"), "Capas");
    assert_eq!(tr_fmt("Sheet {n}", &[("n", "1")]), "Lámina 1");
    assert_eq!(
        tr_fmt("{n} entities selected", &[("n", "5")]),
        "5 entidades seleccionadas"
    );

    // Portuguese translation
    set_language("pt-BR");
    assert_eq!(current_language(), "pt-BR");
    assert_eq!(tr("File"), "Arquivo");
    assert_eq!(tr("Save"), "Salvar");
    assert_eq!(tr("Layers"), "Camadas");

    // Indonesian translation
    set_language("id");
    assert_eq!(current_language(), "id");
    assert_eq!(tr("File"), "File");
    assert_eq!(tr("Save"), "Simpan");

    // Italian translation
    set_language("it");
    assert_eq!(current_language(), "it");
    assert_eq!(tr("File"), "File");
    assert_eq!(tr("Save"), "Salva");

    // Chinese translation
    set_language("zh-CN");
    assert_eq!(current_language(), "zh-CN");
    assert_eq!(tr("File"), "文件");
    assert_eq!(tr("Save"), "保存");

    // Fallback test
    set_language("en");
    assert_eq!(tr("Some Nonexistent String"), "Some Nonexistent String");
}

