#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The Standard §17.1 progress block for Escape Pod, read straight off the
# §17.5 tracking documents:
#
#   M0..M6       ticked required TODO.md tasks per milestone
#   MVP          verified required requirements, M0-M6 (the PRD's MVP line)
#   TODO         ticked required tasks over required tasks
#   PLAN         ticked required PLAN.md work packages
#   PRD          ticked required PRD.md requirement items
#
# Items ending `(optional)` and struck-through `(dropped)` items never count.
# The script also checks that the ticks tell the truth: a PRD item is ticked
# exactly when its requirement is `verified` in doc/requirements.toml, and a
# PLAN package backed by requirements (doc/progress-map.toml) is ticked only
# when every one of them is verified. A mismatch fails the run.

const MILESTONES = {
    M0: "Foundation and compliance"
    M1: "Tap/hold engine"
    M2: "CLI, config and agent surface"
    M3: "evdev backend and safety"
    M4: "X11 fallback backend"
    M5: "Privileges, packaging and manual"
    M6: "Verification and release gate"
}

# Parses a §17.5 document into its task items with milestone and tick.
export def items [path: string]: nothing -> table {
    open --raw $path
    | lines
    | reduce --fold { ms: "" rows: [] } { |line, acc|
        let head = ($line | parse -r '^## (M[0-9]+) ')
        if ($head | is-not-empty) {
            $acc | update ms ($head | get 0.capture0)
        } else {
            let it = ($line | parse -r '^- \[( |x)\] ((?:[PT]-[0-9]{3})|(?:ESC-SRS-[0-9]{3}))(.*)$')
            if ($it | is-empty) or ($acc.ms == "") { $acc } else {
                let i = ($it | get 0)
                $acc | update rows ($acc.rows | append {
                    id: $i.capture1
                    milestone: $acc.ms
                    done: ($i.capture0 == "x")
                    optional: ($i.capture2 =~ '\(optional\)')
                    dropped: ($i.capture2 =~ '\(dropped\)')
                })
            }
        }
    }
    | get rows
}

# Optional items continue onto wrapped lines; marks an item optional when
# `(optional)` appears anywhere before the next item.
def optional-ids [path: string]: nothing -> list<string> {
    open --raw $path
    | split row "\n- ["
    | each { |chunk| $chunk | parse -r '^(?:x| )\] ([PT]-[0-9]{3}|ESC-SRS-[0-9]{3})[\s\S]*\(optional\)' }
    | flatten
    | get -o capture0
    | default []
}

# Renders one §17.1 row: six-character label, twenty cells, percentage in
# five characters ending in column 33, description from column 37.
export def bar [label: string pct: int desc: string]: nothing -> string {
    let filled = if $pct >= 100 { 20 } else if $pct <= 0 { 0 } else {
        let f = (($pct * 20) / 100 | math round)
        if $f >= 20 { 19 } else if $f <= 0 { 1 } else { $f }
    }
    let cells = (("" | fill -c "█" -w $filled) + ("" | fill -c "░" -w (20 - $filled)))
    let pct_s = ($"($pct)%" | fill -a right -w 5)
    $"(($label + ':') | fill -a left -w 6)[($cells)]($pct_s)   ($desc)"
}

# Rounded percentage; an empty denominator reads 100 (nothing remains).
def percent [num: int den: int]: nothing -> int {
    if $den == 0 { 100 } else { ($num * 100) / $den | math round }
}

# Required items only, with the optional flag taken from wrapped lines.
def required [path: string]: nothing -> table {
    let opt = (optional-ids $path)
    items $path | where { |i| not $i.dropped and not $i.optional and $i.id not-in $opt }
}

# Prints the block (or its figures as JSON); fails when a tick lies.
def main [
    --json # Print the figures as JSON on standard output
]: nothing -> nothing {
    let status = (
        open doc/requirements.toml | get requirement
        | reduce --fold {} { |r, acc| $acc | insert $r.id $r.status }
    )
    let todo = (required TODO.md)
    let plan = (required PLAN.md)
    let prd = (required PRD.md)
    let map = (open doc/progress-map.toml | get plan)

    mut fails = []
    for p in $prd {
        let verified = (($status | get -o $p.id) == "verified")
        if $p.done != $verified {
            $fails = ($fails | append $"PRD.md ($p.id) is ticked=($p.done) but its status is ($status | get -o $p.id)")
        }
    }
    for p in $plan {
        let entry = ($map | where id == $p.id)
        if ($entry | is-empty) {
            $fails = ($fails | append $"($p.id) has no row in doc/progress-map.toml")
            continue
        }
        let reqs = ($entry | get 0 | get -o requirements | default [])
        if ($reqs | is-not-empty) {
            let open_reqs = ($reqs | where { |r| ($status | get -o $r) != "verified" })
            if $p.done and ($open_reqs | is-not-empty) {
                $fails = ($fails | append $"PLAN.md ($p.id) is ticked but ($open_reqs | str join ', ') not verified")
            }
            if (not $p.done) and ($open_reqs | is-empty) {
                print -e $"[INFO] progress: PLAN.md ($p.id) can be ticked; every requirement is verified"
            }
        }
    }

    let ms_rows = ($MILESTONES | columns | each { |m|
        let mine = ($todo | where milestone == $m)
        let done = ($mine | where done | length)
        {
            row: $m num: $done den: ($mine | length)
            pct: (percent $done ($mine | length)) desc: ($MILESTONES | get $m)
        }
    })
    let t_done = ($todo | where done | length)
    let p_done = ($plan | where done | length)
    let r_done = ($prd | where done | length)
    let opt_t = ((items TODO.md | length) - ($todo | length))
    let opt_r = ((items PRD.md | length) - ($prd | length))
    let suffix = { |n: int| if $n > 0 { $", +($n) optional" } else { "" } }
    let rows = (
        $ms_rows
        | append { row: MVP num: $r_done den: ($prd | length) pct: (percent $r_done ($prd | length)) desc: "Escape Pod v0.1" }
        | append { row: TODO num: $t_done den: ($todo | length) pct: (percent $t_done ($todo | length)) desc: $"($t_done)/($todo | length) tasks(do $suffix $opt_t)" }
        | append { row: PLAN num: $p_done den: ($plan | length) pct: (percent $p_done ($plan | length)) desc: "PLAN.md, M0–M6" }
        | append { row: PRD num: $r_done den: ($prd | length) pct: (percent $r_done ($prd | length)) desc: $"PRD.md, M0–M6(do $suffix $opt_r)" }
    )
    if $json {
        { project: "Escape Pod" rows: $rows failures: $fails } | to json --raw | print
    } else {
        print "Project: Escape Pod"
        print ""
        for r in $rows { print (bar $r.row $r.pct $r.desc) }
    }
    if ($fails | is-not-empty) {
        for f in $fails { print -e $"[ERROR] progress: ($f)" }
        exit 1
    }
}
