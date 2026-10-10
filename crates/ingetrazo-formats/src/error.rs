// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, FormatError>;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("IGZ format error: {0}")]
    Igz(#[from] crate::igz::IgzError),

    #[error("OBJ format error: {0}")]
    Obj(#[from] crate::obj::ObjError),

    #[error("STL format error: {0}")]
    Stl(#[from] crate::stl::StlError),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Unsupported format: {0}")]
    Unsupported(String),
}
