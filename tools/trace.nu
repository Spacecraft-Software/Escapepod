#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The traceability matrix (Standard §21.3), generated from
# doc/requirements.toml and the evidence markers in the tree, never kept by
# hand. Ported from Operator's tools/trace.nu. Implements: ESC-SRS-085
#
# A marker is `Verifies: ESC-SRS-nnn` on a test or a gate, or
# `Implements: ESC-SRS-nnn` on the item that implements a requirement; one
# marker may list several identifiers separated by commas. Two failures are
# always fatal:
#
#   - a marker citing an identifier the requirement set does not contain;
#   - a requirement whose status is `implemented` or `verified` with no
#     marker behind it, or `verified` with no `Verifies:` marker.
#
# With --release (gate G3) every baselined requirement without a marker is
# an orphan too, and orphans block the release.

const MARKER = '(Verifies|Implements): (ESC-SRS-[0-9]{3}(?:\s*,\s*ESC-SRS-[0-9]{3})*)'
const ROOTS = "{crates,tests,tools,doc,packaging,fuzz,.github}/**/*"
const EXCLUDE = ["**/target/**" "**/fuzz/corpus/**" "**/fuzz/artifacts/**" "doc/requirements.*"]

# Every marker in the evidence roots: one record per cited identifier.
export def markers []: nothing -> table<id: string, kind: string, file: string> {
    glob $ROOTS --exclude $EXCLUDE
    | where { |f| ($f | path type) == "file" }
    | each { |f|
        let text = (try { open --raw $f | into string } catch { "" })
        let rel = ($f | path relative-to $env.PWD)
        $text
        | parse -r $MARKER
        | each { |m|
            $m.capture1
            | split row ","
            | each { |id| { id: ($id | str trim) kind: $m.capture0 file: $rel } }
        }
        | flatten
    }
    | flatten
}

# The matrix: one row per requirement with its evidence files.
export def matrix []: nothing -> table {
    let reqs = (open doc/requirements.toml | get requirement)
    let marks = (markers)
    $reqs | each { |r|
        let mine = ($marks | where id == $r.id)
        {
            id: $r.id
            milestone: $r.milestone
            priority: $r.priority
            method: $r.method
            status: $r.status
            verifies: ($mine | where kind == Verifies | get file | uniq)
            implements: ($mine | where kind == Implements | get file | uniq)
        }
    }
}

# The gate failures for a matrix and a marker set.
export def failures [
    matrix: table
    marks: table
    release: bool
]: nothing -> list<string> {
    let ids = ($matrix | get id)
    let unknown = ($marks | where id not-in $ids | get id | uniq)
    let unknown_f = ($unknown | each { |u| $"marker cites unknown identifier ($u)" })
    let claimed = (
        $matrix
        | where { |r| (
            ($r.status in [implemented verified])
            and ($r.verifies | is-empty) and ($r.implements | is-empty)
        ) }
        | each { |r| $"($r.id) is ($r.status) with no evidence marker" }
    )
    let unverified = (
        $matrix
        | where { |r| $r.status == verified and ($r.verifies | is-empty) }
        | each { |r| $"($r.id) is verified with no Verifies: marker" }
    )
    let orphans = if $release {
        $matrix
        | where { |r| (
            $r.status == baselined and $r.priority != optional
            and ($r.verifies | is-empty) and ($r.implements | is-empty)
        ) }
        | each { |r| $"($r.id) is baselined with no evidence marker \(release orphan\)" }
    } else { [] }
    $unknown_f | append $claimed | append $unverified | append $orphans
}

# Prints the matrix summary (or the matrix as JSON) and fails on a defect.
def main [
    --release # Gate G3: a baselined requirement with no marker is an orphan
    --json # Print the matrix as JSON on standard output
]: nothing -> nothing {
    let marks = (markers)
    let m = (matrix)
    let fails = (failures $m $marks $release)
    if $json {
        {
            generated: (date now | date to-timezone UTC | format date "%Y-%m-%dT%H:%M:%SZ")
            requirements: $m
            failures: $fails
        } | to json --raw | print
    } else {
        let by = ($m | group-by milestone --to-table | each { |g|
            let req = ($g.items | where priority != optional)
            {
                milestone: $g.milestone
                required: ($req | length)
                verified: ($req | where status == verified | length)
                marked: ($req | where { |r| ($r.verifies | is-not-empty) } | length)
            }
        })
        print ($by | sort-by milestone)
    }
    if ($fails | is-not-empty) {
        for f in $fails { print -e $"[ERROR] trace: ($f)" }
        exit 1
    }
    print -e $"[OK] trace: ($m | length) requirements, ($marks | length) markers, no unknown identifier, no unevidenced status"
}
