# Local validation record

Validated on Linux with Rust 1.99.1 and Node.js 24.19.0. Raw command output lives
in `reports/validation/`; research includes its own compiler and host metadata.

| Command/check | Actual outcome | Log |
|---|---|---|
| cargo fmt --check | Passed | fmt.txt |
| cargo clippy --locked --all-targets -- -D warnings | Passed | clippy.txt |
| cargo test --locked | 29 tests passed | rust-tests.txt |
| cargo build --release --locked | Passed | release-build.txt |
| npm run format:check --prefix frontend | Passed after formatting | frontend-format-check.txt |
| npm run typecheck --prefix frontend | Passed | frontend-typecheck.txt |
| npm test --prefix frontend | 6 tests passed | frontend-tests.txt |
| npm run build --prefix frontend | Passed | frontend-build.txt |
| npm run e2e --prefix frontend | 2 tests passed | browser.txt |
| Release research executable | Completed; 5 resilience checks and queue check passed | research.txt |
| README generate/replay/workload commands | Passed; mixed: 822 accepted / 4 alerts | demo-replay.txt, workload-replay.txt |

The Rust suite includes 20 core tests, four API tests, two CLI tests and three
research helper tests. It covers validation, all detectors, ordering, duplicate
accounting, bounded state, malformed recovery, I/O failures, real queue pressure,
API cancellation/conflict/limits, and CLI error/overwrite behavior. No numeric
coverage percentage is claimed.

Browser tests start the actual release backend and built React app. They check
UI/API counts, all four detectors, evidence, JSON download, research display,
invalid seed handling, stop and a 390px mobile viewport without horizontal overflow.
Screenshots in `docs/screenshots/` are genuine browser captures, not mockups.

The browser dependency was installed from Playwright's matching official
headless-shell archive, checked with ZIP CRC verification. The first launch
failed because the extracted executable lacked its execute bit. Setting that
installation permission and specifying PLAYWRIGHT_CHROMIUM_EXECUTABLE allowed
the real tests to run. No blocked security controls were bypassed. Standard
Playwright installation instructions are in README.

Research results are fresh executable output, not copied from earlier transient
runs. Raw cases and repeated trials are retained; no production accuracy or
throughput claim is made. This is not a comprehensive adversarial audit.

Not performed: hosted GitHub Actions execution, external dependency vulnerability
audit, non-Linux validation or non-Chromium browser testing. CI configuration
is supplied without claiming a hosted pass. Clean archive verification is
recorded separately below when finished.

## Clean archive verification

The archive was extracted to a new directory. `cargo build --release --locked`
passed using the shared Cargo build cache. `npm ci` created fresh local Node
dependencies and `npm run build` passed. CLI generate/replay produced 822 accepted
events and four alerts. Both browser scenarios passed again against the extracted
source/build (archive-browser.txt). All 25 source/test/frontend files plus Cargo
manifests matched the delivery source byte-for-byte. Build/dependency folders
are excluded from the ZIP. Desktop/mobile screenshots were visually inspected;
a sidebar text overflow was corrected and screenshots recaptured from this build.
