# fakeobd default vehicle: a Volt-shaped lab bench, entirely synthetic.
#
# What is from volt's documentation: the arbitration IDs (7E0..7E7 physical/functional OBD range and the
# GMLAN pair 244 -> 644 for the instrument cluster), the module names HPCM2, BECM and IPC, the set of DID
# numbers (docs/OBD-CONFIG-DID-APPENDIX.md on branch claude/obd-config-re-887200), the four DID values
# the app's Demo mode returns (marked "demo"), and three DTC numbers from docs/GM-DTC-REFERENCE.md.
# The 2026-11-14 trial (docs/AVD-TRIAL.md) records which header the app used for which identifier.
# The appendix pairs 241 -> 641 and 24C -> 64C. It lists ATSH621 and no ATCRA pair, so 621 is absent.
#
# What is not: a vehicle capture of which module owns a DID, every value below, or a real VIN.
# `$1A 90` repeats the placeholder SYNTH000000000FKE. `$1A` 99, CC, DF, B0, 6F and 6D are synthetic
# tags on the headers the 2026-11-14 scan used, so that scan is not refused before it reaches `$22`.
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
lid 90 ascii SYNTH000000000FKE   # $1A 90. Placeholder, 17 characters, not a VIN.
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
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
lid 90 ascii SYNTH000000000FKE   # the app asks 7E4 for $1A 90 first; same placeholder, not a VIN
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
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
lid 90 ascii SYNTH000000000FKE   # second $1A 90, after ATCRA641; same placeholder, not a VIN
lid 99 ascii SY99
lid CC ascii SYCC
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
lid 90 ascii SYNTH000000000FKE
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D

ecu M7E1 req=7E1 resp=7E9 functional=no pad=00 latency=4
did 2889 0000                   # also asked on 621, which has no recorded response id
lid 90 ascii SYNTH000000000FKE
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D

ecu M7E7 req=7E7 resp=7EF functional=no pad=00 latency=4
# Synthetic lab cell table: 4181-419F then 4200-4240. Value is B000 + index*0020,
# so (uint16 * 5) / 65535 stays above 0. The live Cells tab drops a non-positive reply.
# 41A0-41FF are not in the table.
did 4181 B000
did 4182 B020
did 4183 B040
did 4184 B060
did 4185 B080
did 4186 B0A0
did 4187 B0C0
did 4188 B0E0
did 4189 B100
did 418A B120
did 418B B140
did 418C B160
did 418D B180
did 418E B1A0
did 418F B1C0
did 4190 B1E0
did 4191 B200
did 4192 B220
did 4193 B240
did 4194 B260
did 4195 B280
did 4196 B2A0
did 4197 B2C0
did 4198 B2E0
did 4199 B300
did 419A B320
did 419B B340
did 419C B360
did 419D B380
did 419E B3A0
did 419F B3C0
did 4200 B3E0
did 4201 B400
did 4202 B420
did 4203 B440
did 4204 B460
did 4205 B480
did 4206 B4A0
did 4207 B4C0
did 4208 B4E0
did 4209 B500
did 420A B520
did 420B B540
did 420C B560
did 420D B580
did 420E B5A0
did 420F B5C0
did 4210 B5E0
did 4211 B600
did 4212 B620
did 4213 B640
did 4214 B660
did 4215 B680
did 4216 B6A0
did 4217 B6C0
did 4218 B6E0
did 4219 B700
did 421A B720
did 421B B740
did 421C B760
did 421D B780
did 421E B7A0
did 421F B7C0
did 4220 B7E0
did 4221 B800
did 4222 B820
did 4223 B840
did 4224 B860
did 4225 B880
did 4226 B8A0
did 4227 B8C0
did 4228 B8E0
did 4229 B900
did 422A B920
did 422B B940
did 422C B960
did 422D B980
did 422E B9A0
did 422F B9C0
did 4230 B9E0
did 4231 BA00
did 4232 BA20
did 4233 BA40
did 4234 BA60
did 4235 BA80
did 4236 BAA0
did 4237 BAC0
did 4238 BAE0
did 4239 BB00
did 423A BB20
did 423B BB40
did 423C BB60
did 423D BB80
did 423E BBA0
did 423F BBC0
did 4240 BBE0
lid 90 ascii SYNTH000000000FKE
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
