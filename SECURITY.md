# Security and privacy

Use only authorized input. SentinelFlow is educational local research software,
not a production IDS or forensic acquisition tool. It never connects to input IPs
or emits attack traffic. Included examples are synthetic.

The server binds to 127.0.0.1 and has no authentication/TLS/user isolation. Do not
expose it via a tunnel or reverse proxy. Local Host checks, custom mutation header,
no CORS grants, 16 KiB body cap and shared request budget are defense in depth,
not protection against local processes. Static pages use CSP and nosniff; styles
allow inline chart values. No analytics, external fonts or cloud runtime calls.

The API selects bundled scenarios, not paths/commands. State/histories and JSONL
records are bounded, but malicious files can still consume processing time.
CLI input is opened read-only and checked for regular-file status after opening.
Symlinks are followed; reads may affect atime. Input/output parent directories
must be operator-controlled. This is not a symlink-race hardened sandbox.
Create-new prevents ordinary overwrite; failures may leave partial files.

Reports contain supplied IPs/identifiers: review before sharing. Dependency
installation needs registry access; lockfiles are not a vulnerability audit.
For a suspected vulnerability, preserve a minimal synthetic reproducer and contact
the maintainer privately through the hosting platform when available. Never post
private traffic or secrets publicly. No guaranteed response SLA is offered.
