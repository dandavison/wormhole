wormhole-env() {
    source /tmp/wormhole.env
}

wormhole-cd() {
    if [ -n "$1" ]; then
        builtin cd "$1"
    else
        builtin cd "$WORMHOLE_PROJECT_DIR"
    fi
}

wormhole-switch() {
    dir="${1:-$PWD}"
    dir=$(readlink -f "$dir")
    WORMHOLE_PROJECT_DIR="$dir"
    WORMHOLE_PROJECT_NAME=$(basename "$dir")
    cd "$dir"
}
