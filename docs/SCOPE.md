# Scope

`fakeobd` is a lab bench for the **read side** of vehicle diagnostics. It exists so that an app can be run,
tested and reverse-engineered against a vehicle that cannot be damaged. It inherits the limits recorded in
volt's `docs/OBD-CONFIG-SCOPE.md`, and adds one rule of its own: the policy layer is part of the design, not
a feature that can be switched off by accident.

## Not built, and not to be built here

1. **No SecurityAccess.** `$27` is refused by the policy's hard floor under every configuration
   (`obd_policy::HARD_DENY`). There is no seed generator, no key generator, no seed-to-key algorithm and no
   table of keys. An ECU in this repository never answers a seed request.
2. **No entitlement or paywall tooling.** Nothing that checks, forges or bypasses an app's activation.
3. **No live-vehicle writes.** There is no code path that sends `$2E`, `$2F`, `$31`, `$11`, `$14`,
   `$28`, `$34`-`$37`, `$85` or the like to anything. The simulated ECUs do not implement them; the
   policy refuses them first.
4. **No real adapters, vehicles or phones from the tests or the examples.** The client and capture tools
   connect to loopback only unless told otherwise, and nothing in the repository opens a serial port,
   a Bluetooth device or an ADB connection.
5. **No VINs, Bluetooth addresses, serial numbers or personal paths** in the tree. The VIN-like string in
   the shipped profile is a placeholder that is not a valid VIN.

## What the policy does

- Default: read-only. OBD-II modes 01, 02, 03, 05, 06, 07, 09, 0A; UDS `$10` (sub-functions 01 and 03 only),
  `$19`, `$22`, `$3E`; GMLAN `$1A`. Everything else is refused with a negative response.
- Deny beats allow; a service that is not allowed is denied; an unknown service is denied.
- A configuration may allow more (a relay owner may decide to let `$2E` through to a bench module), except
  `$27`.
- [`profiles/lab-control.policy`](../profiles/lab-control.policy) is a separate file. It keeps the read-only
  preset, including tester present `$3E`, and admits the documented body-control frames on service `AE`.
  The default policy does not admit those frames. The simulated ECU answers only those exact frames with
  one acknowledgement (`EE`) and does not move a mechanism, open a serial port, a Bluetooth radio or an
  ADB device, or change stored data. Any other `AE` payload keeps the negative response it already gets.
  The file cannot allow `$27`. `$2E`, `$31` and `$34`-`$37` stay unimplemented.
- Every request the policy sees, passed or refused, is written to the audit log. A refused request's body
  is not kept past the service, sub-function and one more byte, so a refused `$27` or `$2E` does not leave
  its key or payload in the log.

## Why a simulator and a policy in one repository

The policy library (`obd-policy`) has no dependency on the simulator. The same engine can sit in a relay
between an app and a real adapter and refuse the same services there; see
[architecture.md](architecture.md#reusing-the-policy-in-a-relay). Building it first, inside a simulator that
can be attacked freely, is the point: the refusal table is tested exhaustively against all 256 service
identifiers before anything real depends on it.
