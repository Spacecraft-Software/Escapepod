#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The local CI mirror (PLAN P-006). `.github/workflows/ci.yml` runs these same
# gates, by name, through `nu tools/ci.nu --only <gate>`, so the workflow and
# the mirror cannot disagree, and a task can be ticked on local evidence while
# GitHub Actions is unavailable. A gate is added here and to the workflow in
# the same PR, never later.

# The posture set (Standard §5.2, §5.7, §15.3, §19.5, §26.1).
# Verifies: ESC-SRS-002, ESC-SRS-003, ESC-SRS-005, ESC-SRS-006
const POSTURE = [
    README.md
    AGENTS.md
    CLAUDE.md
    SKILL.md
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

# Runs one external command as a gate; true when it exits 0.
def run-cmd [name: string cmd: closure]: nothing -> bool {
    let run = (do $cmd | complete)
    if $run.exit_code != 0 {
        print -e $"error: gate ($name) failed \(exit ($run.exit_code)\)"
        print -e $run.stdout $run.stderr
    }
    $run.exit_code == 0
}

# Fails when a posture file is missing, or when the GPL text is not a regular
# LICENSE with LICENSES/GPL-3.0-or-later.txt linking to it (§4.3).
def posture-gate []: nothing -> bool {
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
    ($missing | is-empty) and $link_ok
}

# Fails unless README.md and AGENTS.md declare assurance Category B (§19.2).
# PROJECTS.md lives in the Projects repository; its row is inspected in
# doc/records/inspection-m0.md. Verifies: ESC-SRS-004
def category-gate []: nothing -> bool {
    let missing = (
        [README.md AGENTS.md]
        | where { |f| not (open --raw $f | str contains "Category B") }
    )
    if ($missing | is-not-empty) {
        print -e $"error: Category B not declared in ($missing | str join ', ')"
    }
    $missing | is-empty
}

# Fails unless the engine crate forbids unsafe code (Standard §3.1).
# Verifies: ESC-SRS-021
def forbid-gate []: nothing -> bool {
    let ok = (
        open --raw crates/escapepod-core/src/lib.rs
        | lines
        | any { |l| ($l | str trim) == "#![forbid(unsafe_code)]" }
    )
    if not $ok {
        print -e "error: crates/escapepod-core/src/lib.rs must declare #![forbid(unsafe_code)]"
    }
    $ok
}

# Inspects the packaging (Standard §21.1 inspection, run on every change):
# the service account, the sandbox, the no-restart rule, udev rules scoped
# to group escapepod, and no `input` group membership anywhere.
# Verifies: ESC-SRS-069, ESC-SRS-070, ESC-SRS-071, ESC-SRS-072, ESC-SRS-073
export def packaging-findings []: nothing -> list<string> {
    let unit = (open --raw packaging/systemd/escapepod.service | lines | each { str trim })
    let has = { |line: string| $line in $unit }
    let rules = (
        open --raw packaging/udev/70-escapepod.rules | lines
        | where { |l| not ($l | str starts-with "#") and ($l | str trim | is-not-empty) }
    )
    mut found = []
    for want in ["User=escapepod" "RestrictAddressFamilies=AF_UNIX AF_NETLINK" "Restart=on-failure" "RestartPreventExitStatus=6"] {
        if not (do $has $want) { $found = ($found | append $"escapepod.service lacks `($want)`") }
    }
    let groups = ($rules | parse -r 'GROUP="(?<g>[^"]+)"' | get g | append ($rules | parse -r 'g:(?<g>[a-z_-]+):' | get g) | uniq)
    if ($groups != [escapepod]) {
        $found = ($found | append $"udev rules grant groups ($groups | str join ', '); only escapepod is allowed")
    }
    let input = (
        glob "packaging/**/*" | where { |f| ($f | path type) == file }
        | each { |f| open --raw $f | lines | where { |l| (
            ($l =~ 'extraGroups.*"input"') or ($l =~ '^m \S+ input\b')
            or ($l =~ 'usermod .*-aG? *input') or ($l =~ 'gpasswd -a .* input')
        ) } }
        | flatten
    )
    if ($input | is-not-empty) {
        $found = ($found | append $"packaging adds an account to `input`: ($input | str join '; ')")
    }
    $found
}

def packaging-gate []: nothing -> bool {
    let f = (packaging-findings)
    for x in $f { print -e $"error: ($x)" }
    $f | is-empty
}

# The tracked files of the repository at `dir` whose index copy holds a CR
# byte (Standard §6.5). Verifies: ESC-SRS-008
export def crlf-files [dir: string = "."]: nothing -> list<string> {
    ^git -C $dir ls-files --eol
    | lines
    | where { |l| $l =~ 'i/(crlf|mixed)' }
}

# Fails when a tracked text file is stored with a CR byte in the index.
def crlf-gate []: nothing -> bool {
    let bad = (crlf-files)
    if ($bad | is-not-empty) {
        print -e $"error: CR bytes in tracked files:\n($bad | str join "\n")"
    }
    $bad | is-empty
}

# Fails when SKILL.md's frontmatter description exceeds 1000 rendered
# characters (Standard §5.6; the loader hard-fails at 1024).
def skill-gate []: nothing -> bool {
    let length = (
        open SKILL.md --raw
        | split row "---"
        | get 1
        | from yaml
        | get description
        | str length
    )
    if $length > 1000 {
        print -e $"error: SKILL.md description is ($length) characters; the cap is 1000"
    }
    $length <= 1000
}

# Fails when the manual does not build to Info and HTML, or when either
# build prints a warning. PDF needs TeX: the opt-in `pdf` gate and the
# workflow's docs job. Verifies: ESC-SRS-078
def manual-gate []: nothing -> bool {
    let info = (do { ^makeinfo --no-split -o /dev/null doc/escapepod.texi } | complete)
    let html = (
        do { ^texi2any --html --no-split -o /dev/null doc/escapepod.texi } | complete
    )
    let ok = (
        $info.exit_code == 0 and $html.exit_code == 0
        and ($info.stderr | is-empty) and ($html.stderr | is-empty)
    )
    if not $ok {
        print -e "error: gate manual failed"
        print -e $info.stderr $html.stderr
    }
    $ok
}

# Builds the PDF manual with TeX from nixpkgs and fails on any TeX
# warning (overfull, underfull, undefined). Verifies: ESC-SRS-078
def pdf-gate []: nothing -> bool {
    let dir = (^mktemp -d | str trim)
    let texinfo = (^nix build --no-link --print-out-paths nixpkgs#texinfo | str trim)
    let run = (
        with-env { TEXINPUTS: $"($texinfo)/texmf-dist/tex/texinfo:" } {
            do {
                cd doc
                ^nix shell nixpkgs#texliveSmall nixpkgs#texinfo -c texi2pdf --batch $"--build-dir=($dir)/build" -o $"($dir)/escapepod.pdf" escapepod.texi
            } | complete
        }
    )
    let warnings = ($run.stdout + $run.stderr | lines | where { |l| $l =~ '^(Overfull|Underfull)|Warning' })
    rm -rf $dir
    if $run.exit_code != 0 or ($warnings | is-not-empty) {
        print -e $"error: gate pdf failed \(exit ($run.exit_code)\)"
        print -e ($warnings | str join "\n")
    }
    $run.exit_code == 0 and ($warnings | is-empty)
}

# Every gate, in run order. The cargo gates (fmt to deny) are the
# merge-blocking set. Verifies: ESC-SRS-001, ESC-SRS-007
def gates []: nothing -> record {
    {
        posture: { posture-gate }
        category: { category-gate }
        forbid: { forbid-gate }
        crlf: { crlf-gate }
        skill: { skill-gate }
        reuse: { run-cmd reuse { ^reuse lint } }
        srs: { run-cmd srs { ^nu tools/srs.nu --check } }
        packaging: { packaging-gate }
        guix-crates: { run-cmd guix-crates { ^nu tools/guix-crates.nu --check } }
        manual: { manual-gate }
        pdf: { pdf-gate }
        tools: {
            [
                (run-cmd tools { ^nu tools/tests/trace.nu })
                (run-cmd tools { ^nu tools/tests/crlf.nu })
            ] | all { |ok| $ok }
        }
        trace: { run-cmd trace { ^nu tools/trace.nu } }
        progress: { run-cmd progress { ^nu tools/progress.nu } }
        fmt: { run-cmd fmt { ^cargo fmt --all --check } }
        clippy: {
            run-cmd clippy {
                ^cargo clippy --all-targets --all-features --locked -- -D warnings
            }
        }
        test: { run-cmd test { ^cargo test --workspace --locked } }
        doc: {
            run-cmd doc {
                with-env { RUSTDOCFLAGS: "-D warnings" } {
                    ^cargo doc --workspace --no-deps --locked
                }
            }
        }
        audit: { run-cmd audit { ^cargo audit } }
        deny: { run-cmd deny { ^cargo deny check } }
        x11: {
            run-cmd x11 {
                with-env { ESCAPEPOD_X11_REQUIRE: "1" } {
                    (^cargo test -p escapepod --features x11-tests --locked
                        --test x11 -- --test-threads=1)
                }
            }
        }
        vm: { run-cmd vm { ^nix build --no-link -L .#checks.x86_64-linux.nixos } }
        hardware: {
            if ($env.ESCAPEPOD_HW_ON_HOST? | default "") != "1" {
                run-cmd hardware { ^nix build --no-link -L .#checks.x86_64-linux.nixos }
            } else {
                run-cmd hardware {
                    with-env { ESCAPEPOD_HW_REQUIRE: "1" } {
                        (^cargo test -p escapepod --features hardware-tests --locked
                            --test hardware -- --test-threads=1)
                    }
                }
            }
        }
    }
}

# Gates run only when named with --only: hardware needs /dev/uinput and
# /dev/input; pdf fetches TeX through nix; vm builds and boots the NixOS
# VM test. None is in the default run.
const OPT_IN = [hardware pdf vm]

# Runs every gate (or the ones named by --only) and exits 1 if any fails.
def main [
    --only: string # Comma-separated gate names to run, as the workflow does
    --list # Print the gate names and exit
]: nothing -> nothing {
    let all = (gates)
    if $list {
        print ($all | columns | str join "\n")
        return
    }
    let names = if $only == null { $all | columns | where { |g| $g not-in $OPT_IN } } else {
        $only | split row "," | each { str trim }
    }
    let unknown = ($names | where { |n| $n not-in ($all | columns) })
    if ($unknown | is-not-empty) {
        print -e $"error: unknown gate ($unknown | str join ', ')"
        print -e "hint: nu tools/ci.nu --list"
        exit 2
    }
    let results = (
        $names | each { |n| { gate: $n ok: (do ($all | get $n)) } }
    )
    print ($results | update ok { |r| if $r.ok { "pass" } else { "FAIL" } })
    if ($results | any { |r| not $r.ok }) {
        exit 1
    }
}
