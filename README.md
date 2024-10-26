![tach-rs logo 128x128](https://github.com/user-attachments/assets/9798db50-356a-4499-a8b7-4f06003bc9cd)
# tach-rs 
## Rust tachometer built on [Embassy](https://github.com/embassy-rs/embassy)
This repository currently contains only the code, although the 3D models and PCB/Schematics are also going to be released. It works by taking RPM data from the ECU over OBDII and combining that with a separate measurement taken directly from the RPM sensor to get an accurate but more importantly very resilient reading. Battery voltage and coolant temps are also requested from the ECU and displayed on screen.
## Compatibility (Is my car supported?) 
Probably not. This was made for a Daihatsu Hijet S210P mini truck, and with support for the Hijet S110 and S80 coming soon. While I tried to keep the project very modular, I still don't want to re-implement car specific logic for a car I don't have. If you know a little Rust I think modifying a module to support your vehicle would not be too hard (Or at least copying the platform agnostic components). As for the PCB, there are ample GPIO pins broken out to connectors, so hopefully that wont be a problem.
## Demo from first prod installation:
![20241016_132608](https://github.com/user-attachments/assets/0bfb7cfd-8530-4a5e-be97-359b0eb13f98)
![20241016_132629](https://github.com/user-attachments/assets/226086f2-54cc-42f6-b809-54c27dc4537f)
![20241026_102246](https://github.com/user-attachments/assets/38c17a52-7651-4c04-8056-82b24143486f)
[Demo video on youtube](https://youtu.be/AwMxp2c3-qk)
## Questions? 

This project is still in it's early phase and although I have one production installation, I don't have much documentation or feedback. If you want to use my code and have problems or just want to ask something, feel free at paul@fornage.net.
