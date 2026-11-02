;;; SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
;;; SPDX-License-Identifier: GPL-3.0-or-later
;;;
;;; GNU Guix package definition for Escape Pod (Standard §5.5; ESC-SRS-075).
;;; Build with `guix build -f packaging/guix.scm` from the repository root.
;;; Until the first release the source is this checkout (the git-tracked
;;; files); the release (M6, T-102) replaces it with the signed tag's tarball
;;; and its SHA-256. Installs the binary, the Info manual (registered by Guix's
;;; info-dir profile hook), and the systemd unit, udev rule, sysusers and
;;; tmpfiles entries under lib/ for distributions that use them; on Guix
;;; System write a service from them instead.

(use-modules (guix packages)
             (guix gexp)
             (guix git-download)
             (guix build-system cargo)
             ((guix licenses) #:prefix license:)
             (gnu packages acl)
             (gnu packages texinfo))

;; The locked crates.io sources, generated from Cargo.lock by
;; `nu tools/guix-crates.nu`; cargo-build-system builds offline.
(include "guix-crates.scm")

(define %checkout
  (dirname (dirname (current-filename))))

(define-public escapepod
  (package
    (name "escapepod")
    (version "0.0.0")
    (source (local-file %checkout "escapepod-checkout"
                        #:recursive? #t
                        #:select? (git-predicate %checkout)))
    (build-system cargo-build-system)
    (arguments
     (list #:install-source? #f
           ;; A virtual workspace: name the binary crate.
           #:cargo-install-paths ''("crates/escapepod")
           #:phases
           #~(modify-phases %standard-phases
               (add-after 'install 'install-extras
                 (lambda* (#:key inputs outputs #:allow-other-keys)
                   (let* ((out (assoc-ref outputs "out"))
                          (lib (string-append out "/lib")))
                     (invoke "makeinfo" "--no-split" "-o" "escapepod.info"
                             "doc/escapepod.texi")
                     (install-file "escapepod.info" (string-append out "/share/info"))
                     (install-file "packaging/systemd/escapepod.service"
                                   (string-append lib "/systemd/system"))
                     (substitute* (string-append lib "/systemd/system/escapepod.service")
                       (("@BINDIR@") (string-append out "/bin")))
                     (install-file "packaging/udev/70-escapepod.rules"
                                   (string-append lib "/udev/rules.d"))
                     (substitute* (string-append lib "/udev/rules.d/70-escapepod.rules")
                       (("@SETFACL@")
                        (search-input-file inputs "/bin/setfacl")))
                     (install-file "packaging/sysusers.d/escapepod.conf"
                                   (string-append lib "/sysusers.d"))
                     (install-file "packaging/tmpfiles.d/escapepod.conf"
                                   (string-append lib "/tmpfiles.d"))))))))
    (inputs (cons acl escapepod-cargo-inputs))
    (native-inputs (list texinfo))
    (home-page "https://EscapePod.SpacecraftSoftware.org/")
    (synopsis "Tap a key for Escape, hold it for Control")
    (description
     "Escape Pod gives one key two jobs: tapped on its own it sends one key,
held with other keys it acts as a modifier.  It is the successor to xcape and
works on the kernel input layer, so it covers X11, Wayland and the console,
with an X11 fallback for sessions without access to input devices.")
    (license license:gpl3+)))

escapepod
