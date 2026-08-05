param([string]$Bin)

rust-objcopy -O ihex $Bin fw.hex
adafruit-nrfutil dfu genpkg --dev-type 0x0052 --application fw.hex dfu.zip
# Make sure you change 'COM6' to reflect the correct COM port when the RAK19007 baseboard is in bootloader/DFU mode (may change between device)
adafruit-nrfutil dfu serial -pkg dfu.zip -p COM6 -b 115200 --singlebank --touch 1200
