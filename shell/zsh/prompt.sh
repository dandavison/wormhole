# using __git_ps1 from https://github.com/git/git/blob/master/contrib/completion/git-prompt.sh
setopt prompt_subst

wormhole-shell-reset() {
    eval "$(curl -s "localhost:7117/shell?pwd=$PWD")"
}

export GIT_PS1_SHOWDIRTYSTATE=yes
export GIT_PS1_UNSTAGED="અ "
export GIT_PS1_STAGED="જ "

function osc8_link {
    local esc=$'\e'
    # %{...%} marks as non-printing
    print -rn -- "%{${esc}]8;;$1${esc}\\%}$2%{${esc}]8;;${esc}\\%}"
}

function prompt_dir_display {
    local name
    if [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD != $WORMHOLE_PROJECT_DIR ]]; then
        name="${WORMHOLE_PROJECT_NAME}/$(realpath --relative-to="$WORMHOLE_PROJECT_DIR" "$PWD")"
    elif [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD == $WORMHOLE_PROJECT_DIR ]]; then
        name="${WORMHOLE_PROJECT_NAME}"
    else
        print -rn -- "${PWD/#$HOME/~}"
        return
    fi
    if [[ -n $WORMHOLE_JIRA_URL ]]; then
        osc8_link "$WORMHOLE_JIRA_URL" "$name"
    else
        print -rn -- "$name"
    fi
}

function prompt_git_branch {
    local branch=$(__git_ps1 "%s")
    [[ -z $branch ]] && return
    local url
    if [[ -n $WORMHOLE_GITHUB_PR_URL ]]; then
        url="$WORMHOLE_GITHUB_PR_URL"
    elif [[ -n $WORMHOLE_GITHUB_REPO ]]; then
        url="https://github.com/${WORMHOLE_GITHUB_REPO}/compare/${branch}?expand=1"
    fi
    if [[ -n $url ]]; then
        print -rn -- "("; osc8_link "$url" "$branch"; print -rn -- ")"
    else
        print -rn -- "($branch)"
    fi
}

PROMPT='%(?.%{$fg_bold[cyan]%}.%{$fg[red]%})' # Color: cyan if success, red if error
PROMPT+='$(prompt_dir_display)'               # Dir display (must be outside ternary to avoid : conflicts)
PROMPT+='%{$reset_color%}'
PROMPT+='%{$fg[red]%}'
PROMPT+='$(prompt_git_branch)'
PROMPT+='%{$reset_color%} '
