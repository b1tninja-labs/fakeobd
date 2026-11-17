# Break log

Each row: the code was changed (`tools/break_check.py` holds the exact edit), the named test was run, and it failed; the file was then restored. Regenerate with `python tools/break_check.py --write-log`.

| Id | File | Result | Tests that failed |
|---|---|---|---|
| M01 | `crates/obd-policy/src/policy.rs` | KILLED | default_policy_admits_exactly_the_read_set_over_all_256_service_ids |
| M02 | `crates/obd-policy/src/policy.rs` | KILLED | allowing_security_access_is_an_error_not_a_silent_no_op, security_access_is_refused_under_every_policy_that_can_be_built |
| M03 | `crates/obd-policy/src/policy.rs` | KILLED | session_control_admits_only_default_and_extended_for_all_256_sub_functions |
| M04 | `crates/obd-policy/src/policy.rs` | KILLED | deny_beats_allow |
| M05 | `crates/obd-policy/src/audit.rs` | KILLED | audit_does_not_keep_the_body_of_a_state_changing_request |
| M06 | `crates/obd-policy/src/policy.rs` | KILLED | did_deny_ranges_apply_to_every_did_in_a_multi_did_read |
| M07 | `crates/obd-policy/src/line.rs` | KILLED | line_classifier_separates_requests_settings_and_bypasses |
| M08 | `crates/isotp/src/lib.rs` | KILLED | tests::seven_bytes_is_single_and_eight_is_first_plus_consecutive |
| M09 | `crates/isotp/src/lib.rs` | KILLED | tests::sequence_number_wraps_from_f_to_0 |
| M10 | `crates/isotp/src/lib.rs` | KILLED | tests::a_wrong_sequence_number_resets_the_channel |
| M11 | `crates/isotp/src/lib.rs` | KILLED | tests::st_min_decoding |
| M12 | `crates/vehicle/src/ecu.rs` | KILLED | ecu::tests::bitmap_sets_the_msb_for_the_first_pid_and_chains_to_the_next_range |
| M13 | `crates/vehicle/src/ecu.rs` | KILLED | a_physical_only_ecu_ignores_the_functional_id |
| M14 | `crates/vehicle/src/ecu.rs` | KILLED | extended_session_unlocks_the_extended_only_did_and_the_s3_timer_takes_it_back |
| M15 | `crates/vehicle/src/ecu.rs` | KILLED | every_state_changing_request_is_refused_and_audited_and_changes_nothing |
| M16 | `crates/vehicle/src/ecu.rs` | KILLED | block_size_and_stmin_from_the_tester_are_obeyed |
| M17 | `crates/vehicle/src/bus.rs` | KILLED | fault_drop_frame_loses_a_consecutive_frame_so_reassembly_never_completes |
| M18 | `crates/vehicle/src/ecu.rs` | KILLED | fault_delay_and_pending |
| M19 | `crates/vehicle/src/bus.rs` | KILLED | every_golden_transcript_replays_clean |
| M20 | `crates/elm/src/lib.rs` | KILLED | echo_and_linefeed_state_take_effect_after_their_own_line |
| M21 | `crates/elm/src/exchange.rs` | KILLED | physical_header_without_a_matching_receive_filter_gets_nothing |
| M22 | `crates/elm/src/lib.rs` | KILLED | st_sets_the_response_window_and_a_slow_ecu_misses_it |
| M23 | `crates/elm/src/exchange.rs` | KILLED | multi_frame_response_prints_a_length_line_and_numbered_rows_without_headers |
| M24 | `crates/elm/src/exchange.rs` | KILLED | functional_request_collects_every_answer_and_a_trailing_digit_stops_early |
| M25 | `crates/elm/src/exchange.rs` | KILLED | a_lost_consecutive_frame_prints_what_arrived_and_a_data_error |
| M26 | `crates/elm/src/exchange.rs` | KILLED | a_response_longer_than_the_receive_buffer_is_buffer_full |
| M27 | `crates/elm/src/exchange.rs` | KILLED | flow_control_data_that_is_not_a_flow_control_frame_stalls_the_transfer |
| M28 | `crates/elm/src/lib.rs` | KILLED | adapter_reset_mid_session_forgets_state_and_leaves_the_request_unanswered |
| M29 | `crates/elm/src/lib.rs` | KILLED | automatic_protocol_searches_once_then_remembers |
| M30 | `crates/obd-conform/src/lib.rs` | KILLED | tests::normalize_drops_echo_prompt_blanks_and_case |
| M31 | `crates/obd-conform/src/lib.rs` | KILLED | tests::glob_wildcards |
| M32 | `crates/obd-conform/src/lib.rs` | KILLED | tests::a_mismatch_on_an_annotated_entry_is_a_known_gap_and_a_match_makes_the_note_stale |
| M33 | `crates/obd-conform/src/lib.rs` | KILLED | tests::tcp_refuses_a_non_loopback_address_by_default |
| M34 | `crates/fakeobd/src/lib.rs` | KILLED | a_non_loopback_bind_is_refused_by_default |
| M35 | `crates/obd-policy/src/audit.rs` | KILLED | audit_sink_receives_one_json_line_per_decision |
| M36 | `crates/obd-policy/src/filter.rs` | KILLED | decide_agrees_with_the_policy_for_all_256_service_ids |
| M37 | `crates/obd-policy/src/filter.rs` | KILLED | a_rewriter_is_judged_again_and_cannot_smuggle_a_refused_service |
| M38 | `crates/obd-policy/src/filter.rs` | KILLED | lines_are_forwarded_answered_or_refused |
| M39 | `crates/fakeobd/src/lib.rs` | KILLED | a_pipe_answers_without_a_socket_one_chunk_per_line |
| M40 | `crates/fakeobd/src/lib.rs` | KILLED | close_is_idempotent_and_disconnects_the_receiver |
| M41 | `crates/serial-bridge/src/lib.rs` | KILLED | a_relay_between_two_streams_refuses_what_the_policy_refuses |
| M42 | `profiles/volt-lab.profile` | KILLED | trial_addresses_answer_the_requests_the_app_sent |
