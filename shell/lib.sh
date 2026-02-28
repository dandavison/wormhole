wormhole-cd() {
    if [ -n "$1" ]; then
        builtin cd "$1"
    else
        builtin cd "$WORMHOLE_PROJECT_DIR"
    fi
}

wormhole-open-editor() {
    wormhole open --land-in editor "${1:-.}"
}

# A hack to make a shell session switch to a different project, without altering other shell
# sessions (tmux panes) in the same tmux window.
wormhole-shell-switch() {
    dir="${1:-$PWD}"
    dir=$(readlink -f "$dir")
    WORMHOLE_PROJECT_DIR="$dir"
    WORMHOLE_PROJECT_NAME=$(basename "$dir")
    cd "$dir"
}

# Wrap the clap_complete-generated zsh completer to expand ~ before it reaches
# the binary. Without this, PathCompleter receives literal "~" (not a real path)
# and _describe escapes ~ in returned candidates.
if [ -n "$ZSH_VERSION" ] && typeset -f _clap_dynamic_completer_wormhole >/dev/null 2>&1; then
    functions[_clap_dynamic_completer_wormhole_orig]=$functions[_clap_dynamic_completer_wormhole]
    _clap_dynamic_completer_wormhole() {
        if [[ $words[$CURRENT] = \~/* || $words[$CURRENT] = \~ ]]; then
            words[$CURRENT]=${words[$CURRENT]/#\~/$HOME}
            PREFIX=${PREFIX/#\~/$HOME}
        fi
        _clap_dynamic_completer_wormhole_orig "$@"
    }
fi
