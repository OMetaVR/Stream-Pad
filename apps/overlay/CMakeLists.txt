cmake_minimum_required(VERSION 3.21)
project(streampad-overlay VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE RelWithDebInfo)
endif()

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Quick Network Svg Test)
qt_standard_project_setup(REQUIRES 6.8)

# Desktop integrations are optional so the overlay still builds (and falls back to a plain
# top-level window) on systems without KDE's layer-shell-qt / KWindowSystem.
find_package(LayerShellQt 6 QUIET)
find_package(KF6WindowSystem QUIET)
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(WAYLAND_CLIENT QUIET IMPORTED_TARGET wayland-client)
endif()

# Pure model/geometry code, shared by the app and the unit tests.
set(OVERLAY_MODEL_SOURCES
    src/keytable.cpp src/keytable.h
    src/overlaysettings.cpp src/overlaysettings.h
    src/panelgeometry.cpp src/panelgeometry.h
)

qt_add_executable(streampad-overlay
    src/main.cpp
    src/options.cpp src/options.h
    src/paths.cpp src/paths.h
    src/platform.cpp src/platform.h
    src/daemonclient.cpp src/daemonclient.h
    src/mocksource.cpp src/mocksource.h
    src/outputs.cpp src/outputs.h
    src/screenshot.cpp src/screenshot.h
)

set_source_files_properties(qml/Theme.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
qt_add_qml_module(streampad-overlay
    URI StreamPad.Overlay
    VERSION 1.0
    DEPENDENCIES QtQuick
    SOURCES
        ${OVERLAY_MODEL_SOURCES}
        src/store.cpp src/store.h
        src/controller.cpp src/controller.h
        src/panelbackground.cpp src/panelbackground.h
        src/iconprovider.cpp src/iconprovider.h
    QML_FILES
        qml/Main.qml
        qml/Theme.qml
        qml/Panel.qml
        qml/ProfilePage.qml
        qml/ProfileDots.qml
        qml/KeyCap.qml
        qml/KeyLegend.qml
        qml/KeyCover.qml
        qml/ToastStack.qml
        qml/Toast.qml
    RESOURCES
        fonts/InterVariable.ttf
        icons/info.svg
        icons/circle-check.svg
        icons/circle-alert.svg
)
# The generated QML type registrations include the C++ headers by file name.
target_include_directories(streampad-overlay PRIVATE src)
target_link_libraries(streampad-overlay PRIVATE Qt6::Quick Qt6::Network Qt6::Svg)

if(LayerShellQt_FOUND)
    target_link_libraries(streampad-overlay PRIVATE LayerShellQt::Interface)
    target_compile_definitions(streampad-overlay PRIVATE STREAMPAD_HAVE_LAYERSHELL)
endif()
if(KF6WindowSystem_FOUND)
    target_link_libraries(streampad-overlay PRIVATE KF6::WindowSystem)
    target_compile_definitions(streampad-overlay PRIVATE STREAMPAD_HAVE_KWINDOWSYSTEM)
endif()
if(WAYLAND_CLIENT_FOUND)
    target_link_libraries(streampad-overlay PRIVATE PkgConfig::WAYLAND_CLIENT)
    target_compile_definitions(streampad-overlay PRIVATE STREAMPAD_HAVE_WAYLAND_CLIENT)
endif()

target_compile_definitions(streampad-overlay PRIVATE STREAMPAD_OVERLAY_VERSION="${PROJECT_VERSION}")
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(streampad-overlay PRIVATE -Wall -Wextra -Wpedantic)
endif()

enable_testing()
qt_add_executable(tst_panelgeometry test/tst_panelgeometry.cpp ${OVERLAY_MODEL_SOURCES})
target_include_directories(tst_panelgeometry PRIVATE src)
target_link_libraries(tst_panelgeometry PRIVATE Qt6::Gui Qt6::Qml Qt6::Test)
add_test(NAME panelgeometry COMMAND tst_panelgeometry)

install(TARGETS streampad-overlay RUNTIME DESTINATION bin)
