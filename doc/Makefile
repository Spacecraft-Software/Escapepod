# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the Escape Pod manual (Standard §8: make info | html | pdf). POSIX make.
# The Requirements chapter is generated: run `make srs` after editing
# requirements.toml.

MAKEINFO ?= makeinfo
TEXI2ANY ?= texi2any
TEXI2PDF ?= texi2pdf
NU ?= nu

.POSIX:
.PHONY: all info html pdf check clean srs

all: info html

srs:
	cd .. && $(NU) tools/srs.nu

info: escapepod.texi requirements.texi
	$(MAKEINFO) --no-split escapepod.texi

html: escapepod.texi requirements.texi
	$(TEXI2ANY) --html --no-split escapepod.texi

pdf: escapepod.texi requirements.texi
	$(TEXI2PDF) --batch --quiet escapepod.texi

check: escapepod.texi requirements.texi
	$(MAKEINFO) --no-split -o /dev/null escapepod.texi
	$(TEXI2ANY) --html --no-split -o /dev/null escapepod.texi
	cd .. && $(NU) tools/srs.nu --check

clean:
	rm -f escapepod.info escapepod.html escapepod.pdf escapepod.aux escapepod.log \
	  escapepod.toc escapepod.cp escapepod.cps escapepod.fn escapepod.ky \
	  escapepod.pg escapepod.tp escapepod.vr
