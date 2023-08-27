#NoEnv
#Warn
SendMode Input
SetWorkingDir %A_ScriptDir%

profile := 1

profiles := {}
profiles[1] := "Audio"
profiles[2] := "Tasks"
profiles[3] := "Websites"

ShowTooltip(profiles[profile] . "")

~ScrollLock::
SetNumLockState, AlwaysOn
return

#If (!GetKeyState("ScrollLock", "T"))

Numpad1::
Send, {Numpad1}
return

Numpad2::
Send, {Numpad2}
return

Numpad3::
Send, {Numpad3}
return

Numpad4::
Send, {Numpad4}
return

Numpad5::
Send, {Numpad5}
return

Numpad6::
Send, {Numpad6}
return

Numpad7::
Send, {Numpad7}
return

Numpad8::
Send, {Numpad8}
return

Numpad9::
Send, {Numpad9}
return

Numpad0::
Send, {Numpad0}
return

#If

NumpadAdd::
profile++
if (profile > 3)
    profile := 1
ShowTooltip(profiles[profile] . "")
return

NumpadSub::
profile--
if (profile < 1)
    profile := 3
ShowTooltip(profiles[profile] . "")
return

#If (profile = 1)

Numpad5::
send {Media_Play_Pause}
return

Numpad4::
send {Media_Prev}
return

Numpad6::
send {Media_Next}
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

#If

#If (profile = 2)

Numpad1::
IfWinExist, ahk_exe discord.exe
    WinActivate
else
    Run, "C:\Users\butte\AppData\Local\Discord\Update.exe --processStart Discord.exe"
return

Numpad2::
IfWinExist, ahk_exe opera.exe
    WinActivate
else
    Run, "C:\Users\butte\AppData\Local\Programs\Opera GX\launcher.exe"
return

Numpad3::
IfWinExist, ahk_exe steam.exe
    WinActivate
else
    Run, "C:\Program Files (x86)\Steam\steam.exe"
return

Numpad4::
IfWinExist, ahk_exe spotify.exe
    WinActivate
else
    Run, "C:\Users\butte\AppData\Roaming\Spotify\Spotify.exe"
return

Numpad5::
IfWinExist, ahk_exe taskmgr.exe
    WinActivate
else
    Run, taskmgr
return

Numpad6::
SetTitleMatchMode, 2
IfWinExist, ahk_class CabinetWClass
    WinActivate
else
    Run, explorer
return

Numpad7::
IfWinExist, ahk_exe voicemeeter.exe
    WinActivate
else
    Run, "C:\Program Files (x86)\VB\Voicemeeter\voicemeeter8x64.exe"
return

#If

#If (profile = 3)

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
Run, https://www.tv88.to
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

#If

ShowTooltip(message) {
    ToolTip, %message%
    SetTimer, HideTooltip, -1000
}

HideTooltip:
ToolTip
SetTimer, HideTooltip, Off
return
