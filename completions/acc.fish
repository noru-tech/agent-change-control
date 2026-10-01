# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_acc_global_optspecs
    string join \n q/quiet v/verbose no-color h/help V/version
end

function __fish_acc_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_acc_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_acc_using_subcommand
    set -l cmd (__fish_acc_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c acc -n "__fish_acc_needs_command" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_needs_command" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_needs_command" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_needs_command" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_needs_command" -f -a "scan" -d 'Collect facts from a forge and write an evaluated manifest'
complete -c acc -n "__fish_acc_needs_command" -f -a "export" -d 'Export normalized events as JSON for offline evaluation'
complete -c acc -n "__fish_acc_needs_command" -f -a "evaluate" -d 'Evaluate a normalized event export offline'
complete -c acc -n "__fish_acc_needs_command" -f -a "validate" -d 'Validate a manifest: schema, timeline, references and recomputed contents'
complete -c acc -n "__fish_acc_needs_command" -f -a "check" -d 'Enforce policy against a manifest, honoring recorded dispositions'
complete -c acc -n "__fish_acc_needs_command" -f -a "pr" -d 'Collect and evaluate one pull request'
complete -c acc -n "__fish_acc_needs_command" -f -a "doctor" -d 'Check that this environment is ready to collect: version, token, policy file, repository; with --online, the token, the rate limit and whether a newer acc exists (GET only)'
complete -c acc -n "__fish_acc_needs_command" -f -a "completions" -d 'Generate shell completions'
complete -c acc -n "__fish_acc_needs_command" -f -a "manpage" -d 'Generate man pages'
complete -c acc -n "__fish_acc_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github help" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from help" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github help" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from help" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand evaluate" -l conformance-json -d 'Read INPUT as an ACP conformance vector (an object with `events` and `policy`) and print the corpus contract\'s single-line JSON result: verdict, codes, assessments and manifest digest. Exit 0 evaluated, 3 invalid, 4 incomplete. See conformance/README.md'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand evaluate" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand validate" -s f -l format -d 'Result format: `text` (the status line, on stderr) or `json` (a result object, on stdout); inferred from the --output extension when omitted' -r -f -a "text\t'One status line; `table` is an alias'
json\t'One JSON object (RFC 8785 bytes) followed by a newline'"
complete -c acc -n "__fish_acc_using_subcommand validate" -s o -l output -d 'Write the result to FILE instead (the status line in text, the object in JSON)' -r -F
complete -c acc -n "__fish_acc_using_subcommand validate" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand validate" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand validate" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand validate" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand validate" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand check" -l policy -d 'Policy file; overrides the policy embedded in the manifest' -r -F
complete -c acc -n "__fish_acc_using_subcommand check" -l as-of -d 'Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when any finding has a non-open disposition; the machine clock is never consulted' -r
complete -c acc -n "__fish_acc_using_subcommand check" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand check" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand check" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand check" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand check" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand check" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand check" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand pr" -l repo -d 'Repository as OWNER/REPO; defaults to GITHUB_REPOSITORY, else the github.com origin remote of the working directory' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand pr" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand pr" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand pr" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand pr" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand pr" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand pr" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand pr" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand pr" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand pr" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand pr" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand pr" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand doctor" -s f -l format -d 'Report format: `text` (a line per check) or `json` (one object)' -r -f -a "text\t'One status line; `table` is an alias'
json\t'One JSON object (RFC 8785 bytes) followed by a newline'"
complete -c acc -n "__fish_acc_using_subcommand doctor" -l online -d 'Also check the token, the rate limit and the latest release on api.github.com (GET only)'
complete -c acc -n "__fish_acc_using_subcommand doctor" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand doctor" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand doctor" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand doctor" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand doctor" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand completions" -s o -l output -d 'Write the script to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand completions" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand completions" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand completions" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand completions" -s h -l help -d 'Print help'
complete -c acc -n "__fish_acc_using_subcommand completions" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand manpage" -l out-dir -d 'Write one page per command into DIR instead of printing acc.1 to stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand manpage" -s o -l output -d 'Write acc.1 (the top-level page) to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand manpage" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand manpage" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand manpage" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand manpage" -s h -l help -d 'Print help'
complete -c acc -n "__fish_acc_using_subcommand manpage" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "scan" -d 'Collect facts from a forge and write an evaluated manifest'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "export" -d 'Export normalized events as JSON for offline evaluation'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "evaluate" -d 'Evaluate a normalized event export offline'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "validate" -d 'Validate a manifest: schema, timeline, references and recomputed contents'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "check" -d 'Enforce policy against a manifest, honoring recorded dispositions'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "pr" -d 'Collect and evaluate one pull request'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "doctor" -d 'Check that this environment is ready to collect: version, token, policy file, repository; with --online, the token, the rate limit and whether a newer acc exists (GET only)'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "completions" -d 'Generate shell completions'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "manpage" -d 'Generate man pages'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check pr doctor completions manpage help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from scan" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from export" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
