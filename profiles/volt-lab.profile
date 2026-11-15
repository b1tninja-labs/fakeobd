# fakeobd default vehicle: a Volt-shaped lab bench, entirely synthetic.
#
# What is from volt's documentation: the arbitration IDs (7E0..7E7 physical/functional OBD range and the
# GMLAN pair 244 -> 644 for the instrument cluster), the module names HPCM2, BECM and IPC, the set of DID
# numbers (docs/OBD-CONFIG-DID-APPENDIX.md on branch claude/obd-config-re-887200), the four DID values
# the app's Demo mode returns (marked "demo"), and three DTC numbers from docs/GM-DTC-REFERENCE.md.
#
# What is not: which DID lives in which module, every other value, the VIN-like string (a placeholder,
# not a real VIN), the calibration IDs and the Mode 06 entries. Do not read vehicle facts out of this file.

s3 5000

ecu HPCM2 req=7E0 resp=7E8 functional=yes pad=00 latency=3
pid 05 5A                       # coolant temperature, synthetic
pid 0C 1AF8                     # engine speed, synthetic
pid 0D 00                       # vehicle speed
pid 11 40                       # throttle position
pid 42 3390                     # control module voltage
freeze 05 5A
lid 90 ascii SYNTH000000000FKE   # GMLAN $1A 90: identifier the Voltage app asks for first; the VIN-like value is a placeholder
freeze 0C 1AF8
info 02 ascii SYNTH000000000FKE # placeholder, 17 characters, not a VIN
info 04 hex 01 53 49 4D 43 41 4C 30 30 30 30 30 30 30 30 30 31  # one calibration ID, 16 ASCII bytes
info 0A hex 01 48 50 43 4D 32 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 # ECU name HPCM2, 20 bytes
mode06 A1 hex 01 0A 0010 0000 00FF 02 0A 0030 0000 00FF
dtc stored P0A7E
dtc pending P0AE3
did 002F 6C                     # demo
did 41AD 0000                   # demo
did 41C6 00000000               # demo
did 43AF F800                   # demo
did 4184 01F4
did 4185 0A28
did 4202 0000 1388
did 42A0 FF00
did 90B1 00112233
did 90F8 53 49 4D 50 41 52 54 30 30 31
did 90FB 53 59 4E 54 48 45 54 49 43 2D 49 44 45 4E 54 2D 30 30 31 2D 58 59 5A   # 23 bytes: forces a multi-frame $22 answer
did 5005 DE AD ext              # readable only in the extended session

ecu BECM req=7E4 resp=7EC functional=yes pad=00 latency=4
pid 42 3390
pid 5B 8C                       # hybrid battery pack remaining life, synthetic
info 0A hex 01 42 45 43 4D 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
dtc stored P0A0D status=08
did 4210 0E10
did 4211 0E0F
did 4212 0E11

ecu IPC req=244 resp=644 functional=no pad=00 latency=6
did 83E1 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 22 23 24 25 26 27 28 29 2A 2B   # 44 bytes (size from volt docs), content synthetic
did 4368 12
