<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — Compliance
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-10-08
     license:    CC-BY-SA-4.0
     project:    Escape Pod
-->

# Escape Pod — Compliance

Escape Pod declares assurance **Category B** under The Steelbore Standard §19. This file is the
§19.5 tailoring register. A clause absent from the register is applied in full; every entry is
re-read at the G3 release gate.


## Tailoring register

| Clause | Status | Justification | Date |
|--------|--------|---------------|------|
| `spacecraft-agentic-cli` §4 (implicit `--yes` under `AI_AGENT` / `AGENT`) | tailored | `daemon run` grabs every keyboard, so an agent must pass `--yes` explicitly (ESC-SRS-037); the same skill's §7 threat model takes precedence over its §4 default | 2026-10-08 |

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
