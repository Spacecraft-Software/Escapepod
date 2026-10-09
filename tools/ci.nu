#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The local CI mirror (PLAN P-006). Every gate `.github/workflows/ci.yml` runs
# is run here too, in the same order, so a task can be ticked on local
# evidence while GitHub Actions is unavailable. A gate is added here in the
# same PR that adds it to the workflow, never later.

# The posture set (Standard §5.2, §5.7, §15.3, §19.5, §26.1; ESC-SRS-002,
# 005, 006). AGENTS.md, CLAUDE.md and SKILL.md join at P-003.
const POSTURE = [
    README.md
    NOTICE.md
    CONTRIBUTING.md
    LICENSE
    LICENSES
    LICENSES/GPL-3.0-or-later.txt
    LICENSES/CC-BY-SA-4.0.txt
    COMPLIANCE.md
    CREDITS.md
    SECURITY.md
    REUSE.toml
    .gitattributes
    .editorconfig
]

# Runs one external gate, prints its verdict, and returns whether it passed.
def run-gate [
    name: string     # Gate name as shown in the report
    cmd: closure     # The gate; passes when it exits 0
]: nothing -> record<gate: string, ok: bool> {
    let run = (do $cmd | complete)
    let ok = ($run.exit_code == 0)
    if not $ok {
        print -e $"error: gate ($name) failed \(exit ($run.exit_code)\)"
        print -e $run.stdout $run.stderr
    }
    { gate: $name ok: $ok }
}

# Fails when a posture file is missing, or when the GPL text is not a regular
# LICENSE with LICENSES/GPL-3.0-or-later.txt linking to it (§4.3).
def posture-gate []: nothing -> record<gate: string, ok: bool> {
    let missing = ($POSTURE | where { |p| not ($p | path exists) })
    let link_ok = (
        ((ls -l LICENSE | get 0.type) == file)
        and ((ls -l LICENSES/GPL-3.0-or-later.txt | get 0.target) == ../LICENSE)
    )
    if ($missing | is-not-empty) {
        print -e $"error: posture files missing: ($missing | str join ', ')"
    }
    if not $link_ok {
        print -e "error: LICENSE must be a regular file and LICENSES/GPL-3.0-or-later.txt a symlink to ../LICENSE"
    }
    { gate: posture ok: (($missing | is-empty) and $link_ok) }
}

# Fails when a tracked text file is stored with a CR byte in the index
# (Standard §6.5; ESC-SRS-008).
def crlf-gate []: nothing -> record<gate: string, ok: bool> {
    let bad = (
        ^git ls-files --eol
        | lines
        | where { |l| $l =~ 'i/(crlf|mixed)' }
    )
    if ($bad | is-not-empty) {
        print -e $"error: CR bytes in tracked files:\n($bad | str join "\n")"
    }
    { gate: crlf ok: ($bad | is-empty) }
}

# Runs every local gate and exits 1 if any fails.
def main []: nothing -> nothing {
    let results = [
        (posture-gate)
        (crlf-gate)
        (run-gate reuse { ^reuse lint })
    ]
    print ($results | update ok { |r| if $r.ok { "pass" } else { "FAIL" } })
    if ($results | any { |r| not $r.ok }) {
        exit 1
    }
}
