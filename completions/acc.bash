_acc() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="acc"
                ;;
            acc,check)
                cmd="acc__subcmd__check"
                ;;
            acc,completions)
                cmd="acc__subcmd__completions"
                ;;
            acc,doctor)
                cmd="acc__subcmd__doctor"
                ;;
            acc,evaluate)
                cmd="acc__subcmd__evaluate"
                ;;
            acc,export)
                cmd="acc__subcmd__export"
                ;;
            acc,help)
                cmd="acc__subcmd__help"
                ;;
            acc,manpage)
                cmd="acc__subcmd__manpage"
                ;;
            acc,pr)
                cmd="acc__subcmd__pr"
                ;;
            acc,scan)
                cmd="acc__subcmd__scan"
                ;;
            acc,validate)
                cmd="acc__subcmd__validate"
                ;;
            acc__subcmd__export,github)
                cmd="acc__subcmd__export__subcmd__github"
                ;;
            acc__subcmd__export,help)
                cmd="acc__subcmd__export__subcmd__help"
                ;;
            acc__subcmd__export__subcmd__help,github)
                cmd="acc__subcmd__export__subcmd__help__subcmd__github"
                ;;
            acc__subcmd__export__subcmd__help,help)
                cmd="acc__subcmd__export__subcmd__help__subcmd__help"
                ;;
            acc__subcmd__help,check)
                cmd="acc__subcmd__help__subcmd__check"
                ;;
            acc__subcmd__help,completions)
                cmd="acc__subcmd__help__subcmd__completions"
                ;;
            acc__subcmd__help,doctor)
                cmd="acc__subcmd__help__subcmd__doctor"
                ;;
            acc__subcmd__help,evaluate)
                cmd="acc__subcmd__help__subcmd__evaluate"
                ;;
            acc__subcmd__help,export)
                cmd="acc__subcmd__help__subcmd__export"
                ;;
            acc__subcmd__help,help)
                cmd="acc__subcmd__help__subcmd__help"
                ;;
            acc__subcmd__help,manpage)
                cmd="acc__subcmd__help__subcmd__manpage"
                ;;
            acc__subcmd__help,pr)
                cmd="acc__subcmd__help__subcmd__pr"
                ;;
            acc__subcmd__help,scan)
                cmd="acc__subcmd__help__subcmd__scan"
                ;;
            acc__subcmd__help,validate)
                cmd="acc__subcmd__help__subcmd__validate"
                ;;
            acc__subcmd__help__subcmd__export,github)
                cmd="acc__subcmd__help__subcmd__export__subcmd__github"
                ;;
            acc__subcmd__help__subcmd__scan,github)
                cmd="acc__subcmd__help__subcmd__scan__subcmd__github"
                ;;
            acc__subcmd__scan,github)
                cmd="acc__subcmd__scan__subcmd__github"
                ;;
            acc__subcmd__scan,help)
                cmd="acc__subcmd__scan__subcmd__help"
                ;;
            acc__subcmd__scan__subcmd__help,github)
                cmd="acc__subcmd__scan__subcmd__help__subcmd__github"
                ;;
            acc__subcmd__scan__subcmd__help,help)
                cmd="acc__subcmd__scan__subcmd__help__subcmd__help"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        acc)
            opts="-q -v -h -V --quiet --verbose --no-color --help --version scan export evaluate validate check pr doctor completions manpage help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__check)
            opts="-f -o -q -v -h -V --policy --as-of --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --as-of)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__completions)
            opts="-o -q -v -h -V --output --quiet --verbose --no-color --help --version bash elvish fish powershell zsh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__doctor)
            opts="-f -q -v -h -V --online --format --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__evaluate)
            opts="-f -o -q -v -h -V --policy --conformance-json --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__export)
            opts="-f -o -q -v -h -V --since --until --policy --agent-account --agent-trailer --ignore-trailers --agent-trace --attestations --verified-by --verification --agent-vendor --max-pages --format --output --quiet --verbose --no-color --help --version github help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --since)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --until)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trailer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --attestations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verified-by)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verification)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-vendor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-pages)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__export__subcmd__github)
            opts="-f -o -q -v -h -V --since --until --policy --agent-account --agent-trailer --ignore-trailers --agent-trace --attestations --verified-by --verification --agent-vendor --max-pages --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --since)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --until)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trailer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --attestations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verified-by)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verification)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-vendor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-pages)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__export__subcmd__help)
            opts="github help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__export__subcmd__help__subcmd__github)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__export__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help)
            opts="scan export evaluate validate check pr doctor completions manpage help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__check)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__completions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__doctor)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__evaluate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__export)
            opts="github"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__export__subcmd__github)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__manpage)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__pr)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__scan)
            opts="github"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__scan__subcmd__github)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__help__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__manpage)
            opts="-o -q -v -h -V --out-dir --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --out-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__pr)
            opts="-f -o -q -v -h -V --repo --policy --agent-account --agent-trailer --ignore-trailers --agent-trace --attestations --verified-by --verification --agent-vendor --max-pages --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --repo)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trailer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --attestations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verified-by)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verification)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-vendor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-pages)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__scan)
            opts="-f -o -q -v -h -V --since --until --policy --agent-account --agent-trailer --ignore-trailers --agent-trace --attestations --verified-by --verification --agent-vendor --max-pages --format --output --quiet --verbose --no-color --help --version github help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --since)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --until)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trailer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --attestations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verified-by)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verification)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-vendor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-pages)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__scan__subcmd__github)
            opts="-f -o -q -v -h -V --since --until --policy --agent-account --agent-trailer --ignore-trailers --agent-trace --attestations --verified-by --verification --agent-vendor --max-pages --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --since)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --until)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-account)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trailer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-trace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --attestations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verified-by)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --verification)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --agent-vendor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-pages)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "json yaml table sarif in-toto in-toto-jsonl" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__scan__subcmd__help)
            opts="github help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__scan__subcmd__help__subcmd__github)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__scan__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        acc__subcmd__validate)
            opts="-f -o -q -v -h -V --format --output --quiet --verbose --no-color --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "text json" -- "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _acc -o nosort -o bashdefault -o default acc
else
    complete -F _acc -o bashdefault -o default acc
fi
