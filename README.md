# NotPetyaAgain
"Proof of Concept" of a UEFI Petya ransomware 

This project is a module of a school project: Study of the APT group Sandworm (a russian hacker group). I started as reproduction of the NotPetya malware written by Sandworm, but when I realised that the sample I was working with didn't worked on UEFI systems, so I came up with the idea to develop a "modern" version of Petya/NotPetya.
The actual version of the program is more similare to Petya because I implemented recovery by decryption.

## How does it work

### Boot
*This repository does only contain the UEFI application, not the loader. (actually the `scripts` folder contain a minimalist loader in python that I used for testing)*

NotPetya Again, when compiled gives you an EFI image from which 86_64 computer's UEFI firmware can boot from. Only 86_64 because the code does only support this architecture. (For entropy source, I use the `rdrand` intel instruction, which is only available on x86/x86_64).

### Partition destruction

My malware targets Windows OS, to do so I select every NTFS partition that the UEFI firmware detects, then parse the MFT zone of each, and encrypt every fragment of it.

### Ecryption technique

Encryption is basicly with an 256 bit AES key. But to generate the key, I use the Diffie-Hellman key agreement protocol (ECDH).
Attackers public key is hardcoded in the malware, so when the victim generate a secret, it generate the encryption key with it. And for recovery, the newly generated secret's public key is displayed in the ransom note, so the attackers just have to generate the key from it and give it to the victim.

### Protection

To prevent booting on a malicious UEFI application, you simply have to activate Secure Boot, which will check for image signature.
This feature is not activated by default relying on the UEFI specification, so it explains why you can still find some computer without Secure Boot.

Nowadays, if a manufacturer wants to sell a computer with the Windows label on it, he MUST activate Secure Boot by default in the settings. But, if your're assembling your computer yourself, the motherboard you're buying DOESN'T have Secure Boot activated by default !

Anyway if ,for whatever reason, you cannot activate Secure Boot, I'm working on a solution to protect the ESP partion from malicious UEFI application.

And finally, if somehow you've managed to get encrypted by NotPetyaAgain, you can check the [NotPetyaAgain_Decrytor](https://github.com/sven-eliasen/NotPetyaAgain_Decryptor) project where you will find the private key to regenerate the key.


## Disclaimer

I do not keep responsibility of any bad usage of this project.
