#NoEnv
#Warn
SendMode Input
SetWorkingDir %A_ScriptDir%

; App paths - update these specific to your files.
global DiscordPath := "C:\Users\username\AppData\Local\Discord\Update.exe"
global OperaPath := "C:\Users\username\AppData\Local\Programs\Opera GX\launcher.exe"
global SteamPath := "C:\Program Files (x86)\Steam\steam.exe"
global SpotifyPath := "C:\Users\username\AppData\Roaming\Spotify\Spotify.exe"
global VoiceMeeterPath := "C:\Program Files (x86)\VB\Voicemeeter\voicemeeter8x64.exe"
global MovieBoxProPath := "C:\Program Files\MovieBoxPro\MovieBoxPro\MovieBoxPro.exe"

ActivateOrLaunch(exeName, launchPath, extraArgs := "") {
    IfWinExist, ahk_exe %exeName%
        WinActivate
    else {
        if (extraArgs != "")
            Run, %launchPath% %extraArgs%
        else
            Run, %launchPath%
    }
}

global profile := 1
global profiles := {}
profiles[1] := "Audio"
profiles[2] := "Tasks"
profiles[3] := "Websites"
profiles[4] := "Hotswap"
profiles[5] := "Desktops"
global gameMode := false
global currentTooltip := ""

~ScrollLock::
    SetNumLockState, AlwaysOn
    ShowProfileTooltip()
return

#If (GetKeyState("ScrollLock", "T"))
NumpadAdd::
    profile := % Mod(profile, 5) + 1
    ShowProfileTooltip()
return

NumpadSub::
    profile := % Mod(profile - 2 + 5, 5) + 1
    ShowProfileTooltip()
return

; Audio profile
#If (GetKeyState("ScrollLock", "T") and profile = 1)
Numpad5::
send {Media_Play_Pause}
return

Numpad4::
if (IsBrowserActive()) {
    Send, !{Left}
} else {
    send {Media_Prev}
}
return

Numpad6::
if (IsBrowserActive()) {
    Send, !{Right}
} else {
    send {Media_Next}
}
return

Numpad8::
send {Volume_Up}
return

Numpad2::
send {Volume_Down}
return

Numpad0::
send {Volume_Mute}
return

Numpad1::
Send {LWin down}b{LWin up}
if (gameMode) {
    gameMode := false
    ShowTooltip("Game Mode: OFF")
}
return

Numpad3::
gameMode := !gameMode
ShowTooltip(gameMode ? "Game Mode: ON" : "Game Mode: OFF")
return

; Tasks profile - app launcher shortcuts
#If (GetKeyState("ScrollLock", "T") and profile = 2)
Numpad1::
ActivateOrLaunch("discord.exe", DiscordPath, "--processStart Discord.exe")
return

Numpad2::
ActivateOrLaunch("opera.exe", OperaPath)
return

Numpad3::
ActivateOrLaunch("steam.exe", SteamPath)
return

Numpad4::
ActivateOrLaunch("spotify.exe", SpotifyPath)
return

Numpad5::
Run, taskmgr
return

Numpad6::
Run, explorer
return

Numpad7::
ActivateOrLaunch("voicemeeter.exe", VoiceMeeterPath)
return

; Websites profile
#If (GetKeyState("ScrollLock", "T") and profile = 3)
Numpad1::
Run, https://www.youtube.com
return

Numpad2::
Run, https://www.google.com
return

Numpad3::
Run, https://www.9anime.pl
return

Numpad4::
Run, %MovieBoxProPath%
return

Numpad5::
Run, https://www.discord.com
return

Numpad6::
Run, https://www.github.com
return

Numpad7::
Run, https://www.twitch.tv
return

Numpad8::
Run, https://www.kick.com
return

Numpad0::
Run, https://chat.openai.com/
return

; Window movement between monitors
MoveActiveWindow(direction) {
    WinGetActiveStats, winTitle, w, h, currentX, currentY
    
    if (direction = "left") {
        newX := currentX - A_ScreenWidth
        newY := currentY
    } else if (direction = "right") {
        newX := currentX + A_ScreenWidth  
        newY := currentY
    } else if (direction = "up") {
        newX := currentX
        newY := currentY - A_ScreenHeight
    } else if (direction = "down") {
        newX := currentX
        newY := currentY + A_ScreenHeight
    }
    
    WinMove, %winTitle%,, %newX%, %newY%
}

; Hotswap profile - move windows around
#If (GetKeyState("ScrollLock", "T") and profile = 4)
Numpad4::
MoveActiveWindow("left")
return

Numpad6::
MoveActiveWindow("right")
return

Numpad8::
MoveActiveWindow("up")
return

Numpad2::
MoveActiveWindow("down")
return

; Desktops profile
#If (GetKeyState("ScrollLock", "T") and profile = 5)
Numpad1::
Send {LWin down}{Ctrl down}{Shift down}{Left down}
Send {LWin up}{Ctrl up}{Shift up}{Left up}
return

Numpad3::
Send {LWin down}{Ctrl down}{Shift down}{Right down}
Send {LWin up}{Ctrl up}{Shift up}{Right up}
return

Numpad4::
Send {LWin down}{Ctrl down}{Left down}
Send {LWin up}{Ctrl up}{Left up}
return

Numpad6::
Send {LWin down}{Ctrl down}{Right down}
Send {LWin up}{Ctrl up}{Right up}
return

Numpad8::
Send {LWin down}{Ctrl down}d{LWin up}{Ctrl up}
return

Numpad2::
Send {LWin down}{Ctrl down}{F4 down}
Send {LWin up}{Ctrl up}{F4 up}
return

Numpad5::
Send {LWin down}{Tab down}
Send {LWin up}{Tab up}
return

#If

; Game Mode persistent hotkey
XButton2::  ; Mouse5
if (gameMode) {
    Send {LWin down}{Ctrl down}{Alt down}{p down}
    KeyWait, XButton2
    Send {LWin up}{Ctrl up}{Alt up}{p up}
} else {
    Send {XButton2}
}
return

#If (profile = 1)  ; Audio profile
; Audio profile hotkeys
#If (profile = 2)  ; Tasks profile
; Tasks profile hotkeys
#If (profile = 3)  ; Websites profile
; Websites profile hotkeys
#If (profile = 4)  ; Hotswap profile
; Hotswap profile hotkeys
#If

; Update the ShowTooltip function to position the tooltip above the mouse cursor
ShowTooltip(message, duration := 1000) {
    global currentTooltip
    currentTooltip := message
    SetTimer, UpdateTooltipPosition, 10  ; Update position every 10ms
    SetTimer, HideTooltip, -%duration%
}

UpdateTooltipPosition:
    if (currentTooltip != "") {
        CoordMode, Mouse, Screen
        CoordMode, ToolTip, Screen
        MouseGetPos, mouseX, mouseY
        tooltipX := mouseX + 20  ; Position tooltip 20 pixels to the right of the mouse cursor
        ToolTip, %currentTooltip%, tooltipX, mouseY
    }
return

HideTooltip:
    ToolTip
    currentTooltip := ""
    SetTimer, UpdateTooltipPosition, Off
return

ShowProfileTooltip() {
    global profile, profiles
    ShowTooltip("Current Profile: " . profiles[profile], 1500)
}


IsBrowserActive() {
    WinGetActiveTitle, activeWindow
    return (InStr(activeWindow, "Opera")
         or InStr(activeWindow, "Chrome")
         or InStr(activeWindow, "Edge")
         or InStr(activeWindow, "Firefox")
         or InStr(activeWindow, "Brave"))
}

; Help Hotkey
F1::
    helpText := "Current Profile: " . profiles[profile] . "`n`n"
    helpText .= "F1: Show this help`n"
    helpText .= "ScrollLock: Toggle Numpad override`n"
    helpText .= "NumpadAdd/Sub: Change profile`n`n"

    if (profile = 1) {
        helpText .= "Audio Profile Hotkeys:`n"
        helpText .= "Numpad5: Play/Pause`n"
        helpText .= "Numpad4/6: Previous/Next (or Browser Back/Forward)`n"
        helpText .= "Numpad8/2: Volume Up/Down`n"
        helpText .= "Numpad0: Mute`n"
        helpText .= "Numpad1: Windows+B (Disable Game Mode if active)`n"
        helpText .= "Numpad3: Toggle Game Mode`n"
    } else if (profile = 2) {
        helpText .= "Tasks Profile Hotkeys:`n"
        helpText .= "Numpad1: Discord`n"
        helpText .= "Numpad2: Opera`n"
        helpText .= "Numpad3: Steam`n"
        helpText .= "Numpad4: Spotify`n"
        helpText .= "Numpad5: Task Manager`n"
        helpText .= "Numpad6: File Explorer`n"
        helpText .= "Numpad7: VoiceMeeter`n"
    } else if (profile = 3) {
        helpText .= "Websites Profile Hotkeys:`n"
        helpText .= "Numpad1: YouTube`n"+-+-
        helpText .= "Numpad2: Google`n"
        helpText .= "Numpad3: 9anime`n"
        helpText .= "Numpad4: MovieBoxPro`n"
        helpText .= "Numpad5: Discord`n"
        helpText .= "Numpad6: GitHub`n"
        helpText .= "Numpad7: Twitch`n"
        helpText .= "Numpad8: Kick`n"
        helpText .= "Numpad0: ChatGPT`n"
    } else if (profile = 4) {
        helpText .= "Hotswap Profile Hotkeys:`n"
        helpText .= "Numpad4: Move window left`n"
        helpText .= "Numpad6: Move window right`n"
        helpText .= "Numpad8: Move window up`n"
        helpText .= "Numpad2: Move window down`n"
    } else if (profile = 5) {
        helpText .= "Desktops Profile Hotkeys:`n"
        helpText .= "Numpad1: Move window to previous desktop (Ctrl+Win+Shift+Left)`n"
        helpText .= "Numpad3: Move window to next desktop (Ctrl+Win+Shift+Right)`n"
        helpText .= "Numpad4: Switch to left desktop (Ctrl+Win+Left)`n"
        helpText .= "Numpad6: Switch to right desktop (Ctrl+Win+Right)`n"
        helpText .= "Numpad8: Create new desktop (Ctrl+Win+D)`n"
        helpText .= "Numpad2: Close current desktop (Ctrl+Win+F4)`n"
        helpText .= "Numpad5: View all desktops (Win+Tab)`n"
    }

    MsgBox, %helpText%
return

; Suspend Hotkey
^!s::  ; Ctrl+Alt+S to suspend/resume the script
Suspend
if A_IsSuspended
    ShowTooltip("Script Suspended")
else
    ShowTooltip("Script Resumed")
return
