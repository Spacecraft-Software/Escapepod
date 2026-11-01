#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Self-test of tools/trace.nu's gate rules on a fixture matrix (T-022):
# an unknown identifier, a verified row with no marker, and a release
# orphan must each fail; a clean fixture must pass.

use ../trace.nu [failures]

# Builds one fixture matrix row.
def row [
    id: string
    status: string
    verifies: list<string>
    --optional # The row is optional, so never an orphan
]: nothing -> record {
    {
        id: $id milestone: M1 method: test status: $status
        priority: (if $optional { "optional" } else { "mandatory" })
        verifies: $verifies implements: []
    }
}

# Fails the self-test with a message.
def check [name: string ok: bool]: nothing -> nothing {
    if not $ok {
        print -e $"[ERROR] trace self-test: ($name)"
        exit 1
    }
    print -e $"[OK] trace self-test: ($name)"
}

def main []: nothing -> nothing {
    let clean = [
        (row ESC-SRS-001 verified [a.rs])
        (row ESC-SRS-002 baselined [])
        (row ESC-SRS-003 baselined [] --optional)
    ]
    let marks = [{ id: ESC-SRS-001 kind: Verifies file: a.rs }]
    check "clean fixture passes" ((failures $clean $marks false) | is-empty)

    let unknown = ($marks | append { id: ESC-SRS-999 kind: Verifies file: b.rs })
    let f1 = (failures $clean $unknown false)
    check "unknown identifier fails" ($f1 | any { |f| $f =~ 'unknown identifier ESC-SRS-999' })

    let bare = ($clean | append (row ESC-SRS-004 verified []))
    let f2 = (failures $bare $marks false)
    check "markerless verified row fails" ($f2 | any { |f| $f =~ 'ESC-SRS-004 is verified' })

    let f3 = (failures $clean $marks true)
    check "release orphan fails" ($f3 | any { |f| $f =~ 'ESC-SRS-002 .*orphan' })
    check "optional row is never an orphan" ($f3 | all { |f| $f !~ 'ESC-SRS-003' })
}
