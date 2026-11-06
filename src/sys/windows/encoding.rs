// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

/// Encode Windows OS text without replacing unpaired UTF-16 surrogates.
#[inline]
pub(crate) fn encode_wide(text: &std::ffi::OsStr) -> impl Iterator<Item = u16> + '_ {
    std::os::windows::ffi::OsStrExt::encode_wide(text)
}
