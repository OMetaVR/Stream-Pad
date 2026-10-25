#pragma once

#include <QString>

#include <span>

namespace streampad {

// One of the 17 numpad keys (docs/SPEC.md §3.1), mirroring streampad_core::keys.
struct PadKey {
    const char *id;      // serialized KeyId, e.g. "KP4"
    int col, row, colSpan, rowSpan;
    const char *legendMain;
    const char *legendSub; // nullptr when the keycap has no secondary legend
};

std::span<const PadKey> padKeys();

// nullptr when `id` is not a pad key.
const PadKey *findPadKey(QStringView id);

} // namespace streampad
