# hazard-sensor-network-firmware
Working repository for firmware development of Hazard Sensor Network project for ANU Capstone 2026.

For more details on the project visit: https://localhazardnetwork.netlify.app/

This repository contains firmware code for both the sensor node and the repeater. Firmware is written for the RAK4630 module, which houses a Nordic nRF52840 MCU and a Semtech SX1262 LoRa® transceiver, and the sensor hardware, which includes: SOME_SENSOR (Function A), SOME_SENSOR (Function B) and SOME_SENSOR (Function C). The repeater is also meant to intergrate into a network using the MeshCore platform.

There are two separate binaries:
`hazard-sensor` contains the binary for the hazard sensor node including firmware for all the sensors and the RAK module.
`repeater` contains the binary for the repeater node which is the firmware for the RAK module.
