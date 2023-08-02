wormhole-env() {
    source /tmp/wormhole.env
}

wormhole-toggle() {
    local flag=/tmp/wormhole-land-in-tmux
    if [ -e $flag ]; then
        rm $flag
        echo vscode
    else
        touch $flag
        echo tmux
    fi
}
