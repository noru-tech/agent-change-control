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
complete -c acc -n "__fish_acc_needs_command" -f -a "verify" -d 'Gate a deploy: every first-parent commit in a range must be landed by a change that a signed acc attestation shows passing'
complete -c acc -n "__fish_acc_needs_command" -f -a "release" -d 'Attest a release: every first-parent commit in a range, the changes that landed it and their verdicts, as an in-toto Statement about the release artifacts'
complete -c acc -n "__fish_acc_needs_command" -f -a "pr" -d 'Collect and evaluate one pull request'
complete -c acc -n "__fish_acc_needs_command" -f -a "doctor" -d 'Check that this environment is ready to collect: version, token, policy file, repository; with --online, the token, the rate limit and whether a newer acc exists (GET only)'
complete -c acc -n "__fish_acc_needs_command" -f -a "completions" -d 'Generate shell completions'
complete -c acc -n "__fish_acc_needs_command" -f -a "manpage" -d 'Generate man pages'
complete -c acc -n "__fish_acc_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and not __fish_seen_subcommand_from github gitlab help" -a "help" -d 'Print this message or the help of the given subcommand(s)'
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
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from github" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from gitlab" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from help" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from help" -f -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
complete -c acc -n "__fish_acc_using_subcommand scan; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
complete -c acc -n "__fish_acc_using_subcommand export; and not __fish_seen_subcommand_from github gitlab help" -a "help" -d 'Print this message or the help of the given subcommand(s)'
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
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from github" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l since -d 'Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339. Defaults to 30 days before --until' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l until -d 'End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339. Defaults to now. A defaulted window is printed and recorded like an explicit one' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l policy -d 'Policy file (defaults to .agent-change-control/policy.yml when present)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l agent-account -d 'Treat LOGIN as the verified account of AGENT (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l agent-trailer -d 'Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable; extends the built-in vendor registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l agent-trace -d 'Agent Trace record files or directories, bound to commits by vcs.revision (repeatable)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l attestations -d 'Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does not verify signatures; see --verified-by' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l verified-by -d 'Who verified the attestations\' signatures before this run, recorded verbatim (for example "gh attestation verify, run 123"). Without it attestation claims count as declared, not signed' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`): its bundles are loaded as signed attestations with the certificate identity the verifier established (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l agent-vendor -d 'Treat AGENT as built and operated by VENDOR (repeatable; extends the built-in registry)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l max-pages -d 'Maximum pages per list endpoint (100 items each)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l team -d 'Also record this reviewer team\'s members (ORG/SLUG, or a GitLab group path; repeatable; ACC011). The token needs to read the members. Implied for the teams the policy\'s tiers require' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l branch -d 'Also record every update of this protected branch in the window (repeatable; ACC012)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l environment -d 'Also record deployments to this environment as production (repeatable; ACC013)' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l deploy-since -d 'Start of the deployment window (default: the start of the merge window). Keep it later than --since, so the changes a deployment shipped were merged inside the merge window' -r
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s f -l format -d 'Output format; inferred from the --output extension when omitted' -r -f -a "json\t''
yaml\t''
table\t'A plain-text table for people; `text` is an alias'
sarif\t''
in-toto\t'An unsigned in-toto Statement v1 whose predicate is the manifest'
in-toto-jsonl\t'JSON Lines of unsigned in-toto Statements, one per change'"
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s o -l output -d 'Write to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l ignore-trailers -d 'Do not read Co-Authored-By trailers; only declarations and account mappings establish agent authorship'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l checks -d 'Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required checks for merged changes (ACC004). Implied when the policy enables ACC004'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l bypass -d 'Also collect ruleset evaluations of merged changes (ACC005); the token needs repository administration read access. GitLab records none, so ACC005 stays unknown there. Implied when the policy enables ACC005'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l branches -d 'Also record every update of the repository\'s default branch in the window (ACC012). Implied when the policy enables ACC012'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l deployments -d 'Also record successful deployments to production environments and the commits each shipped (ACC013): those the forge marks as production. Implied when the policy enables ACC013'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from gitlab" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from help" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand export; and __fish_seen_subcommand_from help" -f -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
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
complete -c acc -n "__fish_acc_using_subcommand verify" -l range -d 'The commits to cover, as BASE..HEAD: the commit deployed last and the one about to be. Every first-parent commit after BASE up to and including HEAD must be covered' -r
complete -c acc -n "__fish_acc_using_subcommand verify" -l verification -d 'A verifier\'s JSON output (`gh attestation verify --format json`), whose bundles hold acc attestations (repeatable; requires --signer)' -r -F
complete -c acc -n "__fish_acc_using_subcommand verify" -l signer -d 'A certificate identity the gate trusts to sign verdicts, compared exactly with the verifier\'s subject alternative name, for example https://github.com/acme/api/.github/workflows/attest.yml@refs/heads/main (repeatable)' -r
complete -c acc -n "__fish_acc_using_subcommand verify" -l issuer -d 'The OIDC issuer the signer\'s identity must come from, for example https://token.actions.githubusercontent.com' -r
complete -c acc -n "__fish_acc_using_subcommand verify" -l attestations -d 'Signed DSSE envelopes or Sigstore bundles (.json or .jsonl, files or directories) whose signatures the caller has verified (repeatable; requires --verified-by)' -r -F
complete -c acc -n "__fish_acc_using_subcommand verify" -l verified-by -d 'Who verified the --attestations signatures, and against which identity, recorded verbatim in the result' -r
complete -c acc -n "__fish_acc_using_subcommand verify" -l policy -d 'Policy file; overrides the policy embedded in each attestation' -r -F
complete -c acc -n "__fish_acc_using_subcommand verify" -l as-of -d 'Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when a signed finding has a non-open disposition; the machine clock is never consulted' -r
complete -c acc -n "__fish_acc_using_subcommand verify" -l checkout -d 'The local clone the range is read from' -r -F
complete -c acc -n "__fish_acc_using_subcommand verify" -s f -l format -d 'Result format: `text` (one line per commit) or `json` (a result object); inferred from the --output extension when omitted' -r -f -a "text\t'One status line; `table` is an alias'
json\t'One JSON object (RFC 8785 bytes) followed by a newline'"
complete -c acc -n "__fish_acc_using_subcommand verify" -s o -l output -d 'Write the result to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand verify" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand verify" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand verify" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand verify" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c acc -n "__fish_acc_using_subcommand verify" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand release" -l range -d 'The commits the release shipped, as BASE..HEAD: the previous release and this one, for example v1.4.0..v1.5.0' -r
complete -c acc -n "__fish_acc_using_subcommand release" -l subject -d 'A release artifact, as NAME=sha256:HEX (repeatable; at least one): the subjects of the Statement, for example the archive and checksum your build produced' -r
complete -c acc -n "__fish_acc_using_subcommand release" -l policy -d 'Policy file; overrides the policy embedded in each manifest' -r -F
complete -c acc -n "__fish_acc_using_subcommand release" -l as-of -d 'Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when a finding has a non-open disposition; the machine clock is never consulted' -r
complete -c acc -n "__fish_acc_using_subcommand release" -l checkout -d 'The local clone the range is read from' -r -F
complete -c acc -n "__fish_acc_using_subcommand release" -s o -l output -d 'Write the Statement to FILE instead of stdout' -r -F
complete -c acc -n "__fish_acc_using_subcommand release" -s q -l quiet -d 'Suppress status lines on stderr'
complete -c acc -n "__fish_acc_using_subcommand release" -s v -l verbose -d 'Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never changes stdout'
complete -c acc -n "__fish_acc_using_subcommand release" -l no-color -d 'Never color diagnostics. acc\'s own output is never colored; this and a non-empty NO_COLOR environment variable switch off color in help and usage errors too'
complete -c acc -n "__fish_acc_using_subcommand release" -s h -l help -d 'Print help'
complete -c acc -n "__fish_acc_using_subcommand release" -s V -l version -d 'Print version'
complete -c acc -n "__fish_acc_using_subcommand pr" -l repo -d 'Repository as OWNER/REPO (GitHub) or GROUP/…/PROJECT (GitLab); defaults to GITHUB_REPOSITORY or CI_PROJECT_PATH, else the origin remote of the working directory' -r
complete -c acc -n "__fish_acc_using_subcommand pr" -l forge -d 'The forge; defaults to the one GitHub Actions or GitLab CI reports, else the origin remote\'s, else GitHub' -r -f -a "github\t''
gitlab\t''"
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
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "scan" -d 'Collect facts from a forge and write an evaluated manifest'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "export" -d 'Export normalized events as JSON for offline evaluation'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "evaluate" -d 'Evaluate a normalized event export offline'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "validate" -d 'Validate a manifest: schema, timeline, references and recomputed contents'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "check" -d 'Enforce policy against a manifest, honoring recorded dispositions'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "verify" -d 'Gate a deploy: every first-parent commit in a range must be landed by a change that a signed acc attestation shows passing'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "release" -d 'Attest a release: every first-parent commit in a range, the changes that landed it and their verdicts, as an in-toto Statement about the release artifacts'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "pr" -d 'Collect and evaluate one pull request'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "doctor" -d 'Check that this environment is ready to collect: version, token, policy file, repository; with --online, the token, the rate limit and whether a newer acc exists (GET only)'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "completions" -d 'Generate shell completions'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "manpage" -d 'Generate man pages'
complete -c acc -n "__fish_acc_using_subcommand help; and not __fish_seen_subcommand_from scan export evaluate validate check verify release pr doctor completions manpage help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from scan" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from scan" -f -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from export" -f -a "github" -d 'A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none'
complete -c acc -n "__fish_acc_using_subcommand help; and __fish_seen_subcommand_from export" -f -a "gitlab" -d 'A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none'
