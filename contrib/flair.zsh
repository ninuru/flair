[[ $TERM_PROGRAM == flair ]] || return
[[ -z $_FLAIR_INTEGRATION_LOADED ]] || return
typeset -g _FLAIR_INTEGRATION_LOADED=1

autoload -Uz add-zsh-hook

_flair_precmd() {
    local exit_status=$?
    local LC_ALL=C
    local encoded='' character code
    local index
    for (( index = 1; index <= ${#PWD}; index++ )); do
        character=${PWD[index]}
        case $character in
            [a-zA-Z0-9/._~-]) encoded+=$character ;;
            *) printf -v code '%02X' "'$character"; encoded+="%$code" ;;
        esac
    done
    printf '\e]133;D;%d\a\e]133;A\a\e]7;file://%s%s\a' "$exit_status" "$HOST" "$encoded"
}

_flair_preexec() {
    printf '\e]133;C\a'
}

add-zsh-hook precmd _flair_precmd
add-zsh-hook preexec _flair_preexec
PS1+=$'%{\e]133;B\a%}'
