#include "keytable.h"

#include <array>

namespace streampad {

namespace {

constexpr std::array<PadKey, 17> kPadKeys{{
    {"NumLock", 0, 0, 1, 1, "Num", "Lock"},
    {"KPSlash", 1, 0, 1, 1, "/", nullptr},
    {"KPAsterisk", 2, 0, 1, 1, "*", nullptr},
    {"KPMinus", 3, 0, 1, 1, "−", nullptr},
    {"KP7", 0, 1, 1, 1, "7", "Home"},
    {"KP8", 1, 1, 1, 1, "8", "↑"},
    {"KP9", 2, 1, 1, 1, "9", "PgUp"},
    {"KPPlus", 3, 1, 1, 2, "+", nullptr},
    {"KP4", 0, 2, 1, 1, "4", "←"},
    {"KP5", 1, 2, 1, 1, "5", nullptr},
    {"KP6", 2, 2, 1, 1, "6", "→"},
    {"KP1", 0, 3, 1, 1, "1", "End"},
    {"KP2", 1, 3, 1, 1, "2", "↓"},
    {"KP3", 2, 3, 1, 1, "3", "PgDn"},
    {"KPEnter", 3, 3, 1, 2, "Enter", nullptr},
    {"KP0", 0, 4, 2, 1, "0", "Ins"},
    {"KPDot", 2, 4, 1, 1, ".", "Del"},
}};

} // namespace

std::span<const PadKey> padKeys()
{
    return kPadKeys;
}

const PadKey *findPadKey(QStringView id)
{
    for (const PadKey &key : kPadKeys) {
        if (id.compare(QLatin1StringView(key.id), Qt::CaseInsensitive) == 0)
            return &key;
    }
    return nullptr;
}

} // namespace streampad
