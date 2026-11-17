# fakeobd default vehicle: a Volt-shaped lab bench, entirely synthetic.
#
# What is from volt's documentation: the arbitration IDs (7E0..7E7 physical/functional OBD range and the
# GMLAN pair 244 -> 644 for the instrument cluster), the module names HPCM2, BECM and IPC, the set of DID
# numbers (docs/OBD-CONFIG-DID-APPENDIX.md on branch claude/obd-config-re-887200), the four DID values
# the app's Demo mode returns (marked "demo"), and three DTC numbers from docs/GM-DTC-REFERENCE.md.
# The 2026-11-14 trial (docs/AVD-TRIAL.md) records which header the app used for which identifier.
# The appendix pairs 241 -> 641 and 24C -> 64C. It lists ATSH621 and no ATCRA pair, so 621 is absent.
#
# What is not: a vehicle capture of which module owns a DID, every value below, the VIN-like string (one
# placeholder, not a real VIN, repeated where the app asked for $1A 90), the calibration IDs and the
# Mode 06 entries. Lengths other than the demo values and the 44-byte 83E1 block are not recorded; those
# reads answer two synthetic bytes. The added ECUs are not functional: 7DF mode 01 stays HPCM2 and BECM,
# which is what the golden transcripts record. Do not read vehicle facts out of this file.

s3 5000

ecu HPCM2 req=7E0 resp=7E8 functional=yes pad=00 latency=3
pid 05 5A                       # coolant temperature, synthetic
pid 0C 1AF8                     # engine speed, synthetic
pid 0D 00                       # vehicle speed
pid 11 40                       # throttle position
pid 42 3390                     # control module voltage
freeze 05 5A
lid 90 ascii SYNTH000000000FKE   # GMLAN $1A 90. The app asks 7E4 first and 241 second; this copy stays so ATSH7E0 still answers. Placeholder, not a VIN.
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
lid 90 ascii SYNTH000000000FKE   # the app's first $1A 90; same placeholder, not a VIN
dtc stored P0A0D status=08
did 4210 0E10
did 4211 0E0F
did 4212 0E11
# $22 reads the trial logged on ATSH7E4. Demo bytes where the demo log has them. 4368 is one byte, as on the cluster. The rest are two synthetic bytes; those lengths were not recorded.
did 40E9 0000
did 41A3 0000
did 41A6 0000
did 41AD 0000                   # demo
did 41C5 0000
did 41C6 00000000               # demo
did 432D 0000
did 434F 0000
did 4356 0000
did 4368 00
did 4369 0000
did 437D 0000
did 4389 0000
did 43A5 0000
did 43A6 0000
did 43AE 0000
did 43AF F800                   # demo
did 4470 0000
did 45FF 0000
did 8334 0000
did 83E2 0000

ecu IPC req=244 resp=644 functional=no pad=00 latency=6
did 83E1 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 22 23 24 25 26 27 28 29 2A 2B   # 44 bytes (size from volt docs), content synthetic
did 4368 12

# Addresses the 2026-11-14 trial used and this profile did not have. Names are the request id: volt does
# not say which module they are. functional=no, so they do not join the 7DF mode 01 broadcast.

ecu M241 req=241 resp=641 functional=no pad=00 latency=5
lid 90 ascii SYNTH000000000FKE   # second $1A 90, after ATCRA641; same placeholder
lid DF ascii SYNTH1ADF0000001    # the app sets ATFCSD300014 before this read, so the answer is multi-frame
did 8005 0000
did 8090 0000
did 809C 0000
did 8537 0000
did CB31 0000

ecu M24C req=24C resp=64C functional=no pad=00 latency=5
# Present, so a physical request is a negative response rather than silence. No identifier was recorded here.

ecu M7E1 req=7E1 resp=7E9 functional=no pad=00 latency=4
did 2889 0000                   # also asked on 621, which has no recorded response id
lid 99 00
lid CC 00
lid 6F 00
lid 6D 00

ecu M7E7 req=7E7 resp=7EF functional=no pad=00 latency=4
did 4181 0000
lid 99 00
lid CC 00
lid 6F 00
lid 6D 00
