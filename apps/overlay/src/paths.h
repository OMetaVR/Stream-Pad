#pragma once

#include <QString>

// Filesystem locations shared with the daemon (docs/SPEC.md §3.6, streampad_core::paths).
namespace streampad::paths {

// $STREAMPAD_SOCKET, else <runtime dir>/daemon.sock.
QString socketPath();

// Installed shared assets (contains icons/lucide/), or an empty string if none was found.
QString shareDir();

} // namespace streampad::paths
