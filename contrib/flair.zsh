[[ $TERM_PROGRAM == flair ]] || return
[[ $_FLAIR_INTEGRATION_LOADED != 9 ]] || return
typeset -g _FLAIR_INTEGRATION_LOADED=9

autoload -Uz add-zsh-hook

_flair_encode() {
    local LC_ALL=C
    local character code
    local index
    REPLY=''
    for (( index = 1; index <= ${#1}; index++ )); do
        character=${1[index]}
        case $character in
            [a-zA-Z0-9/._~-]) REPLY+=$character ;;
            *) printf -v code '%02X' "'$character"; REPLY+="%$code" ;;
        esac
    done
}

_flair_precmd() {
    local exit_status=$?
    local REPLY user host
    _flair_encode "$HOST"
    host=$REPLY
    _flair_encode "$PWD"
    printf '\e]133;D;%d\a\e]133;A\a\e]7;file://%s%s\a' "$exit_status" "$host" "$REPLY"
    [[ $FLAIR_PROMPT == chips ]] || return 0
    _flair_encode "$USERNAME"
    user=$REPLY
    _flair_encode "${${VIRTUAL_ENV:t}:-$CONDA_DEFAULT_ENV}"
    printf '\e]6973;flair;u=%s;h=%s;v=%s\a' "$user" "$host" "$REPLY"
    RPS1=''
    local separator=''
    case $FLAIR_PROMPT_SEPARATOR in
        chevron) separator='❯ ' ;;
        dollar) separator='$ ' ;;
        percent) separator='%% ' ;;
    esac
    PS1=$_flair_gap$'%{\e]6973;prompt\a%}\n'"$separator"$'%{\e]133;B\a%}'
    _flair_gap=$'\n'
}

_flair_preexec() {
    printf '\e]133;C\a'
}

_flair_line_finish() {
    if [[ -z $BUFFER && $CONTEXT == start ]]; then
        printf '\e]133;C\a'
    fi
}

typeset -g _flair_gap=''

autoload -Uz edit-command-line add-zle-hook-widget
zle -N edit-command-line
for _flair_keymap in emacs viins vicmd; do
    [[ $(bindkey -M $_flair_keymap '^X^E') == *undefined-key ]] \
        && bindkey -M $_flair_keymap '^X^E' edit-command-line
done
unset _flair_keymap

add-zsh-hook precmd _flair_precmd
add-zsh-hook preexec _flair_preexec
add-zle-hook-widget zle-line-finish _flair_line_finish
[[ $FLAIR_PROMPT == chips ]] || PS1+=$'%{\e]133;B\a%}'
