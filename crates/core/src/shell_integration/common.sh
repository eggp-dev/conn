# Loaded only into the initial interactive shell. Never export these functions.
__conn_seq=0
__conn_active=0
__conn_lost=0
__conn_emit() {
    # Bounded disk-backed mailbox: no helper executable, socket, or blocking pipe.
    # Only shell lifecycle hooks call this function, never the terminal reader.
    local ack=0 old_umask
    [[ -d $__conn_dir ]] || return 1
    IFS= read -r ack < "$__conn_dir/ack" || return 1
    case $ack in ''|*[!0-9]*) return 1;; esac
    if (( __conn_seq - ack >= 64 )); then __conn_lost=1; return 1; fi
    if (( ${#2} > 8192 || ${#PWD} > 4096 )); then __conn_lost=1; return 1; fi
    if (( __conn_lost )); then
        __conn_lost=0
        __conn_emit gap '' '' || return 1
        # A gap disables recording. Do not consume another queue slot or claim
        # the current command was delivered after the gap.
        return 1
    fi
    __conn_seq=$((__conn_seq + 1))
    old_umask=$(umask)
    umask 077
    builtin printf '%s\0' 1 "$$" "$__conn_seq" "$1" "$2" "$PWD" "$3" > "$__conn_dir/$__conn_seq.event" 2>/dev/null
    local result=$?
    umask "$old_umask"
    return "$result"
}
