#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Self-test of the CR-byte gate in tools/ci.nu: a scratch repository with
# one LF file passes, and staging a CRLF file makes it fail.
# Verifies: ESC-SRS-008

use ../ci.nu [crlf-files]

def main []: nothing -> nothing {
    let dir = (^mktemp -d | str trim)
    ^git -C $dir init -q
    "one\ntwo\n" | save $"($dir)/lf.txt"
    ^git -C $dir add lf.txt
    let clean = (crlf-files $dir)
    "one\r\ntwo\r\n" | save $"($dir)/crlf.txt"
    ^git -C $dir -c core.autocrlf=false add crlf.txt
    let dirty = (crlf-files $dir)
    rm -rf $dir
    if ($clean | is-not-empty) {
        print -e "[ERROR] crlf self-test: an LF-only index was reported"
        exit 1
    }
    if not ($dirty | any { |l| $l =~ 'crlf.txt' }) {
        print -e "[ERROR] crlf self-test: a staged CRLF file was not reported"
        exit 1
    }
    print -e "[OK] crlf self-test: LF passes, a staged CRLF file fails"
}
