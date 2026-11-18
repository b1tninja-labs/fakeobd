# fakeobd default vehicle: a Volt-shaped lab bench, entirely synthetic.
#
# What is from volt's documentation: the arbitration IDs (7E0..7E7 physical/functional OBD range and the
# GMLAN pair 244 -> 644 for the instrument cluster), the module names HPCM2, BECM and IPC, the set of DID
# numbers (docs/OBD-CONFIG-DID-APPENDIX.md on branch claude/obd-config-re-887200), the four DID values
# the app's Demo mode returns (marked "demo"), and three DTC numbers from docs/GM-DTC-REFERENCE.md.
# The 2026-11-14 trial (docs/AVD-TRIAL.md) records which header the app used for which identifier.
# The appendix pairs 241 -> 641 and 24C -> 64C. It lists ATSH621 and no ATCRA pair, so 621 is absent.
#
# What is not: a vehicle capture of which module owns a DID, every value below, or a captured VIN.
# `$1A 90` and mode 09 PID 02 are the Voltage app's own Demo vehicle literal (volt docs/OBD-CONFIG.md).
# The app reads `$1A 90` on 7E4 first and stays on "Scan was not successful" when that string does not
# decode to a model year. `$1A` 99, CB, CC, DF, B0, 6F and 6D are synthetic tags on the headers the
# 2026-11-14 scan used, so that scan is not refused before it reaches `$22`.
# Nothing here decodes a `42xx` value. Calibration IDs and Mode 06 entries are synthetic. Lengths
# other than the demo values and the 44-byte 83E1 block are not recorded; those `$22` reads answer
# two synthetic bytes. The added ECUs are not functional: 7DF mode 01 stays HPCM2 and BECM, which is
# what the golden transcripts record. Do not read vehicle facts out of this file.

s3 5000

ecu HPCM2 req=7E0 resp=7E8 functional=yes pad=00 latency=3
pid 05 5A                       # coolant temperature, synthetic
pid 0C 1AF8                     # engine speed, synthetic
pid 0D 00                       # vehicle speed
pid 11 40                       # throttle position
pid 42 3390                     # control module voltage
freeze 05 5A
lid 90 ascii 1G1RB6S55HU000000   # $1A 90. The app's Demo vehicle literal, not a captured VIN.
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
freeze 0C 1AF8
info 02 ascii 1G1RB6S55HU000000 # same Demo literal as $1A 90, so the two identity reads agree
info 04 hex 01 53 49 4D 43 41 4C 30 30 30 30 30 30 30 30 30 31  # one calibration ID, 16 ASCII bytes
info 0A hex 01 48 50 43 4D 32 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 # ECU name HPCM2, 20 bytes
mode06 A1 hex 01 0A 0010 0000 00FF 02 0A 0030 0000 00FF
dtc stored P0A7E
dtc pending P0AE3
did 002F 6C                     # Fuel Level: first byte / 255, shown as a percent. 6C is 42%. Gallons are that fraction of the app's US-gallon setting; the lab screen used 8.9.
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
lid 90 ascii 1G1RB6S55HU000000   # the app asks 7E4 for $1A 90 first; same Demo literal
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
dtc stored P0A0D status=08
did 4210 0E10
did 4211 0E0F
did 4212 0E11
# $22 reads the trial logged on ATSH7E4. Scales are the Voltage 2.3.0 decoder, not a standard.
# Demo bytes stay where the demo log has them: 41AD, 41C6, 43AF. 4368 is one byte, as on the cluster.
# 41C5 is four bytes because that parser reads four. The other lengths were not recorded.
did 40E9 0190                   # internal resistance, milliohms: uint16 / 2. 0190 is 200
did 41A3 01C2                   # battery capacity, Ah: uint16 / 10. 01C2 is 45.0. Replaces 45FF when both answer
did 41A6 0000                   # two data bytes as an integer, divided by 64
did 41AD 0000                   # liters used: uint16 * 0.125. Demo bytes stay 0000
did 41C5 00000280               # EV trip: four bytes as an integer, divided by 64. 00000280 is 10.0
did 41C6 00000000               # ICE trip: four bytes as an integer, divided by 64. Demo bytes stay 0
did 432D 0000
did 434F 4100                   # battery temperature, C: first byte - 40. 41 is 25 C, shown as 77 F
did 4356 0000
did 4368 00
did 4369 0000
did 437D 0000
did 4389 0000
did 43A5 0320                   # charge count: uint16. 0320 is 800
did 43A6 0C00                   # isolation, kilohms: first byte * 25. 0C is 300
did 43AE 0000
did 43AF F800                   # state of charge, raw percent: uint16 * 100 / 65535. F800 is 96.9%. Demo bytes
did 4470 0078                   # kWh used: uint16 / 10. 0078 is 12.0
did 45FF 0000                   # capacity candidate, Ah: uint16 / 100. Dropped when 41A3 answers
did 8334 8000                   # state of charge, displayed percent: first byte * 100 / 255. 80 is 50.2%
did 83E2 0000

ecu IPC req=244 resp=644 functional=no pad=00 latency=6
did 83E1 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 22 23 24 25 26 27 28 29 2A 2B   # 44 bytes (size from volt docs), content synthetic
did 4368 12

# Addresses the 2026-11-14 trial used and this profile did not have. Names are the request id: volt does
# not say which module they are. functional=no, so they do not join the 7DF mode 01 broadcast.

ecu M241 req=241 resp=641 functional=no pad=00 latency=5
lid 90 ascii 1G1RB6S55HU000000   # second $1A 90, after ATCRA641; same Demo literal
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001    # the app sets ATFCSD300014 before this read, so the answer is multi-frame
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
did 8005 0000
did 8090 0000
did 809C 0000
did 8537 0000
did CB31 0000

ecu M24C req=24C resp=64C functional=no pad=00 latency=5
# No $1A identifier was recorded on this header. The same synthetic set is here so a scan is not a refusal.
lid 90 ascii 1G1RB6S55HU000000
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D

ecu M7E1 req=7E1 resp=7E9 functional=no pad=00 latency=4
did 2889 0000                   # also asked on 621, which has no recorded response id
lid 90 ascii 1G1RB6S55HU000000
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D

ecu M7E7 req=7E7 resp=7EF functional=no pad=00 latency=4
did 4181 BD6F                   # cell voltage, volts: uint16 * 5 / 65535. BD6F is 3.70. The app asks the next cells as 4181 plus a cell index, on header DACDF1
lid 90 ascii 1G1RB6S55HU000000
lid 99 ascii SY99
lid CC ascii SYCC
lid CB ascii SYCB
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
