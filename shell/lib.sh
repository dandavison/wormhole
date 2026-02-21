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
