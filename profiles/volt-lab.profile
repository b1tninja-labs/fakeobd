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
# other than the demo values, the 44-byte 83E1 block on 244, the three-byte schedules on 7E4,
# the four-byte charge-history words, the four-byte 2439 reason, and the four-byte 41C5 and 41C6
# trips are not recorded; the other `$22` reads answer two synthetic bytes. The added
# ECUs are not functional: 7DF mode 01 stays
# HPCM2 and BECM, which is what the golden transcripts record. Do not read vehicle facts out of this file.

s3 5000

ecu HPCM2 req=7E0 resp=7E8 functional=yes pad=00 latency=3
pid 05 5A                       # coolant temperature, synthetic
pid 0C 1AF8                     # engine speed, synthetic
pid 0D 00                       # vehicle speed
pid 11 40                       # throttle position
pid 42 3390                     # control module voltage
freeze 05 5A
# g0 calls B0("11") and then reads these three. 1C47 is already answered below. Module 11 is this 7E0.
did 1C1E 0100                   # code 1, normal high temp
did 1C38 8000                   # first byte * 100 / 255
did 1C39 8000                   # first byte >= 128 is true
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
did 1C47 6400                   # first byte / 10, synthetic
did 0042 03E8                   # uint16 / 1000, synthetic
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
# i0 sends these before it selects module 17. The powertrain group leaves that header at 7E0.
did 000D 2800                   # speed: first byte 40. A first byte of 255 is forced to 0
did 000C 0C80                   # ICE rpm: uint16 / 4 = 800
did 0005 5A00                   # coolant: first byte - 40 = 50 C

ecu BECM req=7E4 resp=7EC functional=yes pad=00 latency=4
pid 42 3390
pid 5B 8C                       # hybrid battery pack remaining life, synthetic
info 0A hex 01 42 45 43 4D 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
# U, V, W, and Z call A0("CB") before these. g0 calls B0("CB") before 41B0. Module CB is this 7E4.
did 0042 03E8                   # uint16 / 1000, synthetic
did 1C43 4100                   # first byte - 40, synthetic
did 41B0 0FA0                   # uint16 / 1000, synthetic
did 41B7 07D0                   # uint16 / 1000, synthetic
did 4357 8000                   # first byte / 2.55, synthetic
did 436F 4100                   # first byte - 40, synthetic
did 4372 0001                   # second byte 1 is Open
did 439B 0100                   # first byte 1 is Connected
did 441E 0800                   # bit 3 set
did 4424 8500                   # (uint16 - 32768) / 128, synthetic
did 4425 3C00                   # first byte - 40, synthetic
did 43CB 01000000               # byte 0 bit 0, full charge
did 43CC 02000000               # byte 0 bit 1, time of day inhibit
did 43CD 00010000               # byte 1 bit 0, low severity fault
did 43CE 00000200               # byte 2 bit 1, hybrid pack protection
did 43CF 00000001               # byte 3 bit 0, max charge time
did 452D 8000                   # first byte / 2.55, synthetic
did 452E 4000                   # first byte / 2.55, synthetic
did 452F 0000                   # 0 is Passed
did 4530 0000                   # 0 is Passed
did 4531 0100                   # 1 is L1
did 4532 0100                   # 1 is Ready
did 4533 6400                   # first byte * 0.3, synthetic
did 4545 B400                   # first byte * 0.05, synthetic
did 4546 8000                   # first byte / 2.55, synthetic
did 8375 020A00                 # weekday 2, 10:00
did 8377 021200                 # weekday 2, 18:00
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
did 40E9 0190                   # uint16 / 2, synthetic
did 41A3 01C2                   # uint16 / 10, synthetic
did 41A6 1018                   # uint16 / 64 km, synthetic
did 41AD 0000                   # demo
did 41C5 00000280               # four bytes / 64, synthetic
did 41C6 00000000               # demo
did 41DA 00C8                   # signed int16 * 0.52, synthetic
# R sends these on the header Y left, which is this 7E4. First byte minus 1 indexes the 7E7 cell ramp.
did 432A 0100                   # index 0, cell 4181
did 432C 2100                   # index 32, cell 4201, a higher positive voltage than 4181
did 432D 0000
did 434F 4100                   # first byte - 40, synthetic
did 4356 0000
did 4368 0A                     # byte * 2 in the 4369 power product
did 4369 3200                   # first byte * 0.2, synthetic
did 436B 0190                   # uint16 / 2, synthetic
did 436C 00C8                   # signed int16 / 20, synthetic
did 437D 0000
did 4389 0000
did 43A5 0320                   # uint16, synthetic
did 43A6 0C00                   # first byte * 25, synthetic
did 43AE 0000
did 43AF F800                   # demo
did 4470 0078                   # uint16 / 10, synthetic
did 45FF 0000
did 8334 8000                   # first byte * 100 / 255, synthetic
did 83E1 010800                 # weekday 1, 08:00; the 44-byte copy stays on 244
did 83E2 010C1E                 # weekday 1, 12:30

ecu IPC req=244 resp=644 functional=no pad=00 latency=6
did 83E1 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 22 23 24 25 26 27 28 29 2A 2B   # 44 bytes (size from volt docs), content synthetic
did 4368 12
# r0 calls A0("80") then 228000 when single-wire is on. Module 80 is this 244.
did 8000 0000                   # synthetic positive, scale not recorded

# Addresses the 2026-11-14 trial used and this profile did not have. Names are the request id: volt does
# not say which module they are. functional=no, so they do not join the 7DF mode 01 broadcast.

ecu M241 req=241 resp=641 functional=no pad=00 latency=5
# h0, g0, and n0 call B0("40") before these $22 reads. Module 40 is this 241. n0's pressures are 225008 through 225005.
did 5005 0640                   # uint16 / 1600, then the pressure unit
did 5006 0640                   # same scale as 5005
did 5007 0640                   # high byte with 5008 low byte, then / 1600
did 5008 0640                   # uint16 / 1600, then the pressure unit
did 900D 0100                   # 1 is power take-off
did 9025 0000                   # first byte stored, no named scale
did 9037 0000                   # first byte stored, no named scale
did 90B1 0FA0                   # uint16 / 1000, then * 0.976
did 90F8 4000                   # signed int16 * 100 / 32767
did 90FB 0100                   # signed int16 / 128
# X sends these on the header h0 left. Module 40 is this 241.
did 801F 6400                   # first byte / 2, then - 40, synthetic
did 82B2 0F00                   # first byte * 2.7, synthetic
did 817B 0E00                   # first byte * 14, synthetic
did 41B4 07D0                   # uint16, synthetic
did 41B2 03E8                   # uint16, synthetic
did 82B5 01F4                   # uint16, synthetic
did 451E 8000                   # first byte / 2.55, synthetic
lid 90 ascii SYNTH000000000FKE   # second $1A 90, after ATCRA641; same placeholder, not a VIN
lid 01 ascii SY01 SY02 SY03 SY04   # sensor ids; the app removes spaces
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
# Module 17 on an 11-bit session. 2414 is a signed int16 / 20. 2439 is four payload bytes.
did 2414 00C8                   # +10 A
did 2439 00000800               # byte 2 bit 3: ignition-switch start
did 2C05 8000                   # aux pump: first byte / 2.55
did 002C 8000                   # first byte / 2.55, synthetic
did 1154 4100                   # first byte - 40, synthetic
did 1940 4100                   # first byte - 40, synthetic
did 3040 01F4                   # uint16 / 10, then - 40, synthetic
did 3041 01F4                   # uint16 / 10, then - 40, synthetic
# The next i0 on this screen asks 7E1. The previous i0 ended on A0("17").
did 000D 2800                   # speed: first byte 40
did 000C 0C80                   # ICE rpm: uint16 / 4 = 800
did 0005 5A00                   # coolant: first byte - 40 = 50 C
# a0 and b0 are later in that same list, so this pass already selected module 17.
did 43AF F800                   # same bytes as BECM
did 8334 8000                   # first byte * 100 / 255
did 41A6 1018                   # uint16 / 64
did 24AC 0014                   # uint16 * 2, synthetic
did 801F 6400                   # first byte / 2, then - 40, synthetic
did 434F 4100                   # first byte - 40, synthetic
did 41A4 4100                   # first byte - 40, synthetic
did 435A 0400                   # first byte * 25, synthetic
did 41B6 0100                   # uint16, synthetic
did 439E 8000                   # first byte / 2.55, synthetic
did 0042 03E8                   # uint16 / 1000, synthetic
did 40D7 4100                   # first byte - 40, synthetic
did 40D9 4100                   # first byte - 40, synthetic
did 40DB 4100                   # first byte - 40, synthetic
did 40DD 4100                   # first byte - 40, synthetic
did 40DF 4100                   # first byte - 40, synthetic
did 40E1 4100                   # first byte - 40, synthetic
# T joins these four and does not select a module. R then joins 432A, 432C, 8334, and 43AF.
did 4470 0078                   # uint16 / 10, same bytes as BECM
did 41C5 00000280               # four bytes / 64, same bytes as BECM
did 41C6 00000000               # four bytes / 64, same bytes as BECM
did 41AD 0000                   # uint16 * 0.125, same bytes as BECM
did 432A 0100                   # index 0, so the follow-up read is 4181
did 432C 2100                   # index 32, so the follow-up read is 4201
did 4181 B000                   # (uint16 * 5) / 65535 stays positive
did 4201 B400                   # index 32 in the cell table
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
# The cell-97 probe appends 4241-4250. Index continues, so the value stays B000 + index*0020.
did 4241 BC00
did 4242 BC20
did 4243 BC40
did 4244 BC60
did 4245 BC80
did 4246 BCA0
did 4247 BCC0
did 4248 BCE0
did 4249 BD00
did 424A BD20
did 424B BD40
did 424C BD60
did 424D BD80
did 424E BDA0
did 424F BDC0
did 4250 BDE0
# Q selects module CD. With the 29-bit flag clear that header is 7E7.
# 82A3 is the first byte times 2. 40D4 is a signed int16 / 20, then negated.
did 82A3 B400                   # 360 V
did 40D4 FE70                   # raw -400, negated to +20 A
# Battery-plus reads this header does not have yet. Scales match the 7E1 lines.
did 24AC 0014                   # uint16 * 2, synthetic
did 0042 03E8                   # uint16 / 1000, synthetic
did 40D7 4100                   # first byte - 40, synthetic
did 40D9 4100                   # first byte - 40, synthetic
did 40DB 4100                   # first byte - 40, synthetic
did 40DD 4100                   # first byte - 40, synthetic
did 40DF 4100                   # first byte - 40, synthetic
did 40E1 4100                   # first byte - 40, synthetic
did 41A4 4100                   # first byte - 40, synthetic
did 435A 0400                   # first byte * 25, synthetic
did 41B6 0100                   # uint16, synthetic
did 439E 8000                   # first byte / 2.55, synthetic
lid 90 ascii SYNTH000000000FKE
lid 99 ascii SY99
lid CC ascii SYCC
lid DF ascii SYNTH1ADF0000001
lid B0 ascii SYB0
lid 6F ascii SY6F
lid 6D ascii SY6D
