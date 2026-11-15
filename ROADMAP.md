# fakeobd roadmap

Obligations of the simulator, with an honest status against the code at `HEAD`. Rows are
`Status | Obligation | Anchor | Test`, the grammar `projecthub sync` reads for Rust projects.

Status:

- `[ok]` implemented, and the named test exists, passes, and **failed when the code it covers was
  deliberately broken**. The break is a numbered mutation (`Mxx`) in `tools/break_check.py`; results in
  [docs/BREAK-LOG.md](docs/BREAK-LOG.md). Anchor is a project decision or a document that was read.
- `[unread]` implemented, tested and break-checked, but the anchor is a standard or datasheet that has not been
  read (none of them is in `primarysource`). The test proves the code agrees with itself, not with the
  standard. Each has an entry in [docs/ground-truth-gaps.md](docs/ground-truth-gaps.md).
- `[open]` not done, not proven, or needs hardware or a human decision.

Test paths are `crate::file::test` (integration tests in `crates/<crate>/tests/`).

## 1. Safety policy (`obd-policy`)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | The default policy admits exactly the read set (modes 01 02 03 05 06 07 09 0A, `$10` 01/03, `$19`, `$1A`, `$22`, `$3E`) over all 256 service identifiers, and refuses everything else | volt `docs/OBD-CONFIG-SCOPE.md` section 5; project design | `obd-policy::policy::default_policy_admits_exactly_the_read_set_over_all_256_service_ids`, `::every_state_changing_service_is_refused_by_default` (M01) |
| [ok] | `$27` SecurityAccess is refused for every sub-function under every policy that can be built, and `allow 27` is an error | volt `docs/OBD-CONFIG-SCOPE.md` section 1 | `obd-policy::policy::security_access_is_refused_under_every_policy_that_can_be_built`, `::allowing_security_access_is_an_error_not_a_silent_no_op` (M02) |
| [ok] | Deny rules beat allow rules; DID deny ranges apply to every DID of a multi-DID read, inclusive at both ends | project design | `obd-policy::policy::deny_beats_allow` (M04), `::did_deny_ranges_apply_to_every_did_in_a_multi_did_read` (M06) |
| [ok] | A configuration may allow a write service other than `$27` (the floor is one service), and the policy file parser rejects what it does not understand | project design | `obd-policy::policy::a_wide_policy_does_admit_a_write_when_the_owner_asks_for_it`, `::parse_rejects_what_it_does_not_understand` |
| [ok] | Every decision, passed or refused, is audited in order; a ring bounds memory but keeps counting; a sink receives one JSON line per decision; a state-changing request's body is not kept past service, sub-function and one byte | project design | `obd-policy::policy::audit_records_passed_and_refused_requests_in_order`, `::audit_ring_drops_the_oldest_but_keeps_counting`, `::audit_does_not_keep_the_body_of_a_state_changing_request` (M05), `::audit_sink_receives_one_json_line_per_decision` (M35) |
| [unread] | `$10` admits sub-functions 01 (default) and 03 (extended) only, with the suppress-positive-response bit masked | ISO 14229-1 (not read) | `obd-policy::policy::session_control_admits_only_default_and_extended_for_all_256_sub_functions` (M03) |
| [unread] | The classification of service identifiers (OBD modes below `$10`, UDS and GMLAN above) and the choice of refusal codes (`33`, `11`, `12`, `31`) | ISO 14229-1, SAE J1979, GMLAN (none read); volt `docs/OBD-CONFIG-DID-APPENDIX.md` for the identifiers the app uses | `obd-policy::policy::nrc_follows_the_kind_of_refusal_and_can_be_overridden`; `obd-policy::service::tests::every_listed_state_changer_classifies_as_one` |
| [unread] | The ELM line classifier separates requests, adapter settings and bypass commands (`ATCAF0`, `ATPP`, `STPX`, ...) | `obdlink-re/docs/COMMANDS.md`; vendor manuals not read | `obd-policy::policy::line_classifier_separates_requests_settings_and_bypasses` (M07) |
| [open] | A relay between an app and a real adapter using `obd-policy`: refuse `Bypass` lines and denied requests, forward the rest, audit all | [docs/architecture.md](docs/architecture.md#reusing-the-policy-in-a-relay) | none: not built |

## 2. ISO-TP (`isotp`)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [unread] | Segmentation: a message of 7 bytes or fewer is a single frame, longer is a first frame (6 bytes, 12-bit length) plus consecutive frames (7 bytes); every length 1..=300 and 4095 round-trips | ISO 15765-2 (not read) | `isotp::tests::every_length_round_trips_padded_and_unpadded`, `::seven_bytes_is_single_and_eight_is_first_plus_consecutive` (M08) |
| [unread] | Consecutive-frame sequence number wraps from 15 to 0; a wrong number resets the channel | ISO 15765-2 (not read) | `isotp::tests::sequence_number_wraps_from_f_to_0` (M09), `::a_wrong_sequence_number_resets_the_channel` (M10) |
| [unread] | STmin decoding (0 to 127 ms, 100 to 900 us, reserved values as 127 ms); flow-control encode and decode; malformed frames refused | ISO 15765-2 (not read) | `isotp::tests::st_min_decoding` (M11), `::flow_control_encodes_and_decodes`, `::malformed_frames_are_refused` |
| [open] | Timeouts (N_Bs, N_Cr), a wait-frame limit in the ECU sender, 29-bit and extended addressing, CAN FD | ISO 15765-2 (not read) | none |

## 3. Vehicle (`vehicle`)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | Every request is judged by the policy before any service code runs; a refusal comes from the policy and is audited; a refused request changes nothing | `docs/SCOPE.md` | `vehicle::bus::every_state_changing_request_is_refused_and_audited_and_changes_nothing` (M15), `::a_policy_that_allows_a_write_still_gets_service_not_supported_from_the_ecu` |
| [ok] | Physical requests reach one ECU; the functional ID reaches every ECU marked functional and no other; functional requests are not answered with the suppressed codes | project design; ISO 14229-1 for the suppression (not read, G-FUNC-NRC) | `vehicle::bus::physical_request_is_answered_by_that_ecu_only`, `::functional_request_is_answered_by_every_functional_ecu_and_not_the_cluster`, `::a_physical_only_ecu_ignores_the_functional_id` (M13), `::functional_requests_do_not_get_the_suppressed_negative_responses` |
| [unread] | Mode 01/02/09/06 supported-ID bitmaps, chained ranges, multi-PID requests, unsupported PIDs silent | SAE J1979 (not read) | `vehicle::bus::supported_pid_bitmap_matches_the_profile`, `::an_unsupported_pid_gets_no_answer_and_several_pids_share_one_answer`, `::mode_06_and_mode_02`, `vehicle::ecu::tests::bitmap_sets_the_msb_for_the_first_pid_and_chains_to_the_next_range` (M12) |
| [unread] | Modes 03, 07, 0A (count byte, then codes); `$19` sub-functions 01 and 02; DTC encoding of P/C/B/U codes | SAE J1979, SAE J2012, ISO 14229-1 (none read); volt `docs/GM-DTC-REFERENCE.md` for the three codes | `vehicle::bus::dtc_services`, `vehicle::profile::tests::dtc_text_round_trips_every_code`, `::known_dtc_encodings` |
| [unread] | Mode 09 multi-frame answers (VIN-style, calibration ID, ECU name) with flow control; block size and STmin from the tester are obeyed | ISO 15765-2, SAE J1979 (not read) | `vehicle::bus::mode_09_vin_is_multi_frame_and_needs_flow_control`, `::block_size_and_stmin_from_the_tester_are_obeyed` (M16), `::a_tester_overflow_flow_control_abandons_the_transfer` |
| [unread] | `$22` reads (single, multi-DID, multi-frame answer and request, unknown DID `31`, wrong length `13`); the extended-session-only DID; the S3 timer; `$3E`; suppress-positive-response; GMLAN `$1A` | ISO 14229-1 (not read); volt capture for the `7F 22 31` frame | `vehicle::bus::read_did_single_and_multi_frame_and_unknown`, `::extended_session_unlocks_the_extended_only_did_and_the_s3_timer_takes_it_back` (M14), `::session_control_variants`, `::gmlan_read_data_by_identifier_answers_from_the_lid_table` |
| [ok] | Fault injection: silent ECU, per-request scoping, drop a frame, bad sequence number, delay, response pending, CAN error, ECU flow-control overflow, wait and block size | project design | `vehicle::bus::fault_silent_and_request_scoping`, `::fault_drop_frame_loses_a_consecutive_frame_so_reassembly_never_completes` (M17), `::fault_bad_sequence_breaks_reassembly`, `::fault_delay_and_pending` (M18), `::fault_can_error_fails_the_transmit_for_one_request_only`, `::flow_control_faults_when_the_ecu_receives_a_multi_frame_request`; `vehicle::fault::tests::parses_each_kind` |
| [ok] | Malformed frames are ignored, not answered | project design | `vehicle::bus::malformed_frames_are_ignored_not_answered` |
| [ok] | The profile parser names the line of an error; the shipped profile parses | project design | `vehicle::profile::tests::errors_name_the_line`, `::the_shipped_profile_parses_and_has_three_ecus` |
| [open] | Module addresses and DIDs the Voltage app uses that the profile lacks (`241`/`641`, `621`, `24C`/`64C`, `7E1`, `7E7`) and the true DID-to-module assignment | volt docs catalog DIDs, not owners; [docs/AVD-TRIAL.md](docs/AVD-TRIAL.md) | none: needs a vehicle or more evidence |
| [open] | Real values, scalings and units for the DIDs; GMLAN services beyond `$1A`; real Mode 06 test IDs | volt `docs/GM-OBD-REFERENCES.md` (GM PDFs, not read here) | none |

## 4. Adapter (`elm`)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [unread] | Echo, linefeed, spaces, headers state: a setting takes effect after its own line is echoed; `ATZ` restores power-on state and reprints the banner | ELM327 datasheet (not read); volt `tools/fake_elm327.py` for the verified dialogue | `elm::adapter::echo_and_linefeed_state_take_effect_after_their_own_line` (M20), `::atz_restores_power_on_state_and_prints_the_banner`, `::stopped_follows_the_linefeed_setting` |
| [unread] | Unknown commands and `ST` on a plain ELM327 answer `?`; an STN profile answers the `ST` family; spaces and case in commands are ignored | ELM327 datasheet, `obdlink-re/docs/COMMANDS.md` | `elm::adapter::unknown_commands_and_st_on_a_plain_elm327_are_question_marks`, `::stn_answers_the_st_family_and_identifies_as_an_elm_clone`, `::spaces_and_case_in_commands_are_ignored` |
| [unread] | Single-frame output formats; multi-frame output (length line and numbered rows without headers, raw frames with headers) | ELM327 datasheet (not read), G-MULTI-FORMAT | `elm::adapter::single_frame_formats_for_spaces_headers`, `::multi_frame_response_prints_a_length_line_and_numbered_rows_without_headers` (M23), `::multi_frame_response_with_headers_lists_every_raw_frame` |
| [unread] | Protocol selection and automatic search (`SEARCHING...`, `A6`, `UNABLE TO CONNECT` for protocols the bus cannot carry) | ELM327 datasheet (not read), G-SEARCH, G-UNABLE-NONCAN | `elm::adapter::automatic_protocol_searches_once_then_remembers` (M29), `::automatic_search_with_nothing_on_the_bus_is_unable_to_connect`, `::protocols_other_than_can_11_500_cannot_connect` |
| [unread] | Receive filter: default 7E8..7EF, `ATCRA`, `ATCF`/`ATCM` | ELM327 datasheet (not read), G-CRA-DEFAULT | `elm::adapter::physical_header_without_a_matching_receive_filter_gets_nothing` (M21), `::cf_and_cm_build_a_filter` |
| [unread] | Response window: `ATST hh` is `hh * 4` ms; a slow ECU misses it and a frame after the window is dropped; a trailing digit stops after N responses | ELM327 datasheet (not read), G-TIMEOUT | `elm::adapter::st_sets_the_response_window_and_a_slow_ecu_misses_it` (M22), `::functional_request_collects_every_answer_and_a_trailing_digit_stops_early` (M24), `::response_pending_is_printed_and_the_real_answer_follows_if_in_time` |
| [unread] | Error replies: `CAN ERROR`, `<DATA ERROR` for a lost or mis-sequenced frame, `BUFFER FULL` (receive limit, long input line), `?` | ELM327 datasheet (not read), G-DATA-ERROR, G-BUFFER-FULL | `elm::adapter::can_error_is_reported_once`, `::a_lost_consecutive_frame_prints_what_arrived_and_a_data_error` (M25), `::a_response_longer_than_the_receive_buffer_is_buffer_full` (M26), `::an_over_long_input_line_is_buffer_full` |
| [unread] | Flow control the adapter sends: default, block size, `ATFCSM`/`ATFCSD`/`ATFCSH`, STN `STCFCPA` pairs; bad flow-control data stalls the transfer; the ECU's flow control for a multi-frame request is obeyed (overflow, waits, block size) | ELM327 datasheet (not read), G-FC-DERIVE | `elm::adapter::flow_control_data_from_atfcsd_is_what_the_ecu_sees`, `::flow_control_data_that_is_not_a_flow_control_frame_stalls_the_transfer` (M27), `::a_multi_frame_request_obeys_the_ecus_flow_control`, `::stn_flow_control_pairs_choose_the_flow_control_address` |
| [unread] | A bench adapter with nothing on the bus reports `CAN ERROR` for a frame nobody acknowledges | G-CAN-ERROR-BENCH | `fakeobd::golden::every_golden_transcript_replays_clean` over `transcripts/bench.txt` (M19) |
| [unread] | A bare carriage return repeats the last command; raw mode `ATCAF0` | ELM327 datasheet (not read), G-REPEAT-CR, G-RAW-MODE | `elm::adapter::a_bare_carriage_return_repeats_the_last_command`, `::raw_mode_sends_the_data_field_as_given_and_prints_whole_frames` |
| [ok] | Adapter faults: reset after N lines (banner, state reset, request unanswered), drop the connection | project design | `elm::adapter::adapter_reset_mid_session_forgets_state_and_leaves_the_request_unanswered` (M28), `::adapter_can_drop_the_connection`, `fakeobd::server::the_adapter_can_drop_the_connection_mid_session` |
| [ok] | Policy refusals reach the client as negative responses through the adapter; session timers run on virtual time across requests | project design | `elm::adapter::uds_reads_pass_the_adapter_and_state_changes_come_back_refused`, `::session_timers_run_on_virtual_time_across_requests` |
| [open] | `ATAT` adaptive timing, `ATR0`, `ATAL`/`ATAR`/`ATM`, `ATPP`, monitor mode, 29-bit and other buses, periodic traffic: acknowledged or absent | G-ACK-ONLY, G-29BIT, G-PERIODIC | none |

## 5. Conformance harness (`obd-conform`)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | The transcript format round-trips every byte (escapes), renders and parses back identically, and rejects malformed files with the line number | [docs/transcript-format.md](docs/transcript-format.md) | `obd-conform::tests::escape_round_trips_every_byte`, `::render_parse_round_trip`, `::parse_errors_name_the_line` |
| [ok] | `normal` comparison ignores echo, linefeeds, blanks, the prompt and case; `exact` sees bytes; `glob` needs equal line counts | project design | `obd-conform::tests::normalize_drops_echo_prompt_blanks_and_case` (M30), `::linefeed_and_echo_differences_are_not_failures_in_normal_mode`, `::exact_mode_sees_the_bytes`, `::glob_wildcards` (M31) |
| [ok] | Known gaps: a mismatch on an annotated entry is a known gap, a match makes the note stale; `strict` turns gaps into failures; replay and compare count failures and catch reordering | project design | `obd-conform::tests::a_mismatch_on_an_annotated_entry_is_a_known_gap_and_a_match_makes_the_note_stale` (M32), `::strict_turns_known_gaps_into_failures`, `::replay_counts_failures_and_gaps`, `::compare_detects_reordering_and_missing_entries` |
| [ok] | The client refuses a non-loopback address unless told; the server refuses to bind one | [docs/SCOPE.md](docs/SCOPE.md) | `obd-conform::tests::tcp_refuses_a_non_loopback_address_by_default` (M33), `fakeobd::server::a_non_loopback_bind_is_refused_by_default` (M34) |
| [ok] | Every golden transcript replays clean, strictly, against a fresh simulator; a changed reply fails; every gap id used is documented | [docs/ground-truth-gaps.md](docs/ground-truth-gaps.md) | `fakeobd::golden::every_golden_transcript_replays_clean`, `::a_changed_reply_is_a_failure_not_a_pass`, `::every_gap_id_in_a_transcript_is_documented`, `::goldens_are_in_canonical_form` |
| [ok] | Over TCP: the dialogue, independent connections, banner only on request, the audit file gets one line per request and not the payload of a refusal, `STOPPED` in realtime mode | project design | `fakeobd::server::a_client_sees_the_full_dialogue_over_tcp`, `::connections_are_independent_vehicles`, `::the_banner_is_sent_on_connect_only_when_asked_for`, `::the_audit_sink_gets_one_line_per_request_and_refusals_are_marked`, `::realtime_mode_answers_stopped_when_a_byte_arrives_during_a_request` |
| [open] | Capture `bench.script` from a real known-good adapter (an OBDLink MX+ or other STN) behind a loopback bridge with nothing on the bus, compare with `transcripts/bench.txt`, close the gaps | [docs/transcript-format.md](docs/transcript-format.md) | none: needs the adapter; never run by an agent on the owner's adapter or phone |
| [open] | Vehicle-scope capture from a real car or a bench module with a CAN simulator | G-ECU-DATA | none: needs a vehicle |

## 6. End to end

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | The Voltage app in the emulator connects over WiFi to `10.0.2.2:35000` and runs its initialisation against the simulator. **Observed once by hand on 2026-11-14**: status Connected, about 2,500 request lines in a minute, app kept cycling | [docs/AVD-TRIAL.md](docs/AVD-TRIAL.md), volt `docs/AVD-OBD-BRIDGE.md` | none: not automated, needs the emulator and the paid app |
| [open] | The app reaches a data screen (VIN detected, live values) against the simulator | [docs/AVD-TRIAL.md](docs/AVD-TRIAL.md) | none: the simulator lacks modules the app addresses |
| [open] | The same app through `obd-bridge` and a real adapter, and through the policy relay | volt `docs/AVD-OBD-BRIDGE.md` | none: needs hardware and the relay |

## 8. Streams and the relay API

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `Filter::decide(request_bytes, Context) -> Action::{Allow, Deny{sid, nrc}, Rewrite}` agrees with `Policy` for all 256 service identifiers, gives the negative response PDU, and feeds every decision to audit sinks with its origin | project design; [docs/architecture.md](docs/architecture.md#reusing-the-policy-in-a-relay) | `obd-policy::filter::decide_agrees_with_the_policy_for_all_256_service_ids` (M36), `::a_deny_carries_the_negative_response`, `::sinks_see_every_decision_with_the_origin` |
| [ok] | A rewriter's output is judged again, so a rewrite cannot smuggle a refused service (`$27`) past the policy | `docs/SCOPE.md` | `obd-policy::filter::a_rewriter_is_judged_again_and_cannot_smuggle_a_refused_service` (M37) |
| [ok] | `Filter::decide_line` forwards adapter settings and allowed requests, answers a denied request with `7F <sid> <nrc>`, refuses bypass and malformed lines, and keeps the response-count digit on a rewrite | project design | `obd-policy::filter::lines_are_forwarded_answered_or_refused` (M38), `::a_rewritten_line_keeps_the_response_count_digit` |
| [ok] | `SimPipe`: the simulator in process, no socket: one reply chunk per line, partial lines wait, take-once receiver, idempotent close, an adapter drop fault closes it | project design | `fakeobd::pipe::a_pipe_answers_without_a_socket_one_chunk_per_line` (M39), `::close_is_idempotent_and_disconnects_the_receiver` (M40), `::an_adapter_drop_fault_closes_the_pipe` |
| [ok] | `crates/serial-bridge` (separate manifest, path dependency on gattmirror's `serial-stream` at its gm-binary worktree): `SimStream` is a `ByteStream`; `PolicyStream` relays between any two `ByteStream`s and answers refusals itself without forwarding them | gattmirror `serial-stream` (commit 40ed77b) | `serial-bridge::bridge::the_simulator_is_a_byte_stream`, `::a_relay_between_two_streams_refuses_what_the_policy_refuses` (M41); not in CI: the path dependency points at a worktree until serial-stream lands on gattmirror main |
| [open] | A relay binary in front of a real adapter; local refusals honour the backend's echo and linefeed state | [docs/architecture.md](docs/architecture.md#reusing-the-policy-in-a-relay) | none: hardware; gattmirror's relay binary is the consumer |

## 7. Registration

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | Registered in `PORTFOLIO.md` and projecthub (tier, wave, GitHub issues for the `[open]` rows) | [docs/REGISTRATION.md](docs/REGISTRATION.md) | none: owner of the workspace root |
