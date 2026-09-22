[[ -o interactive ]] || return
[[ -z ${__conn_loaded-} ]] || return
__conn_loaded=1
__conn_preexec() {
    # $1 is the complete command entered at the prompt, before alias expansion.
    [[ -n $1 ]] || return 0
    __conn_emit start "$1" '' && __conn_active=$__conn_seq
    return 0
}
__conn_precmd() {
    local result=$?
    if (( __conn_active )); then __conn_emit end "$__conn_active" "$result"; fi
    __conn_active=0
    __conn_emit prompt '' ''
    return 0
}
preexec_functions=(__conn_preexec ${preexec_functions:#__conn_preexec})
precmd_functions=(__conn_precmd ${precmd_functions:#__conn_precmd})
__conn_emit ready zsh ''
