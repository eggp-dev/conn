# PROMPT_COMMAND bootstraps us after normal Bash startup, without replacing rc files.
[[ $- == *i* ]] || return
[[ -z ${__conn_loaded-} ]] || return
__conn_loaded=1
export -n PROMPT_COMMAND
PROMPT_COMMAND=$__conn_saved_prompt
if [[ -n ${__conn_prior_debug-} || -n $(trap -p DEBUG) || -o functrace ]] || shopt -q extdebug; then
    __conn_emit unavailable debug_trap ''
    return
fi
__conn_at_prompt=0
__conn_hist=0
__conn_history() {
    local record
    record=$(HISTTIMEFORMAT= builtin history 1)
    if [[ $record =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.*)$ ]]; then
        __conn_number=${BASH_REMATCH[1]}
        # Remove history's fixed separator, preserving command indentation/newlines.
        record=${record#*${BASH_REMATCH[1]}}
        __conn_command=${record#'  '}
    else
        __conn_number=0
        __conn_command=''
    fi
}
__conn_precmd() {
    local result=$?
    __conn_at_prompt=0
    if (( __conn_active )); then __conn_emit end "$__conn_active" "$result"; fi
    __conn_active=0
    __conn_history
    __conn_hist=$__conn_number
    return "$result"
}
__conn_arm() { __conn_at_prompt=1; }
__conn_debug() {
    [[ $__conn_at_prompt == 1 && $BASH_COMMAND != __conn_precmd* ]] || return 0
    __conn_at_prompt=0
    # Never infer a command from BASH_COMMAND, screen contents, or typed bytes.
    # If history did not retain a fresh complete entry, skip it.
    __conn_history
    if (( __conn_number > __conn_hist )) && [[ -n $__conn_command ]]; then
        __conn_emit start "$__conn_command" '' && __conn_active=$__conn_seq
    fi
    return 0
}
__conn_history
__conn_hist=$__conn_number
# Bash 3.2 uses a scalar; keep the user's prompt commands and their exit status.
PROMPT_COMMAND="__conn_precmd
$PROMPT_COMMAND
__conn_arm"
trap '__conn_debug' DEBUG
__conn_emit ready bash ''
__conn_arm
