# NotPetyaAgain
"Proof of Concept" of a UEFI Petya ransomware 

This project is a module of a school project: Study of the APT group Sandworm (a russian hacker group). I started as reproduction of the NotPetya malware written by Sandworm, but when I realised that the sample I was working with didn't worked on UEFI systems, so I came up with the idea to develop a "modern" version of Petya/NotPetya.
The actual version of the program is more similare to Petya because I implemented recovery by decryption.

## How does it work

### Boot
*This repository does only contain the UEFI application, not the loader. (actually the `scripts` folder contain a minimalist loader in python)*

NotPetya Again, when compiled gives you EFI image from which 86_64 computer's UEFI firmware can boot from. Only 86_64 because the code does only support this architecture, like `rdrand` intel instruction for entropy source.

So in my case I'm overwriting the Windows Boot Manager with my EFI image.

### Partition destruction

My malware targets Windows OS, to do so I'm attack every NTFS partition that the UEFI firmware detects.
I then parse the MFT zone of each and encrypt every fragment of it.

### Ecryption technique

Encryption is basicly with an 256 bit AES key. But to generate the key, I use the Diffie-Hellman key agreement protocol (ECDH).
Attackers public key is hardcoded in the malware, so when the victim generate a secret, it generate a the key with it. And for recovery, the newly generated secret public key is displayed in the ransom note, so the attackers just have to generate the key from it and give it to the victim.

## Warranty

I do not keep responsibility of any bad usage of this project.
