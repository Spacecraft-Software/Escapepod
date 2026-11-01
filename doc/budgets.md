<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Escape Pod — budget readings

Measured readings against the ceilings declared at G2 (manual, `Design` → `Budgets`; Standard
§22). The reference machine is in the manual's `Terminology`. Margins are
`(ceiling - measured) / ceiling`, banded per §22.3. The G3 gate replaces the estimates below with
end-to-end readings (T-096, T-097).

## Engine share of the latency budget (ESC-SRS-079)

| Date | Commit | Reading | Ceiling | Share of ceiling |
|---|---|---|---|---|
| 2026-11-01 | M1 engine PR | 65 ns median per `Session::on_key` (`cargo bench -p escapepod-core`, 31 runs of 100 000 events, the reference workload's mix) | 1 ms p99 added latency, end to end | 0.007% |

The engine is not the latency budget's risk; the uinput write and the kernel round trip are,
and they are measured at M6.
