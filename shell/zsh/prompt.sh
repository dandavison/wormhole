# Based on oh-my-zsh/themes/robbyrussell.zsh-theme
# But using __git_ps1 from https://github.com/git/git/blob/master/contrib/completion/git-prompt.sh
setopt prompt_subst

export GIT_PS1_SHOWDIRTYSTATE=yes
export GIT_PS1_UNSTAGED="અ "
export GIT_PS1_STAGED="જ "

function osc8_link {
    printf '\e]8;;%s\e\\%s\e]8;;\e\\' "$1" "$2"
}

function prompt_dir_display {
    local name
    if [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD != $WORMHOLE_PROJECT_DIR ]]; then
        name="${WORMHOLE_PROJECT_NAME}/$(realpath --relative-to="$WORMHOLE_PROJECT_DIR" "$PWD")"
    elif [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD == $WORMHOLE_PROJECT_DIR ]]; then
        name="${WORMHOLE_PROJECT_NAME}"
    else
        echo -n "${PWD/#$HOME/~}"
        return
    fi
    if [[ -n $WORMHOLE_JIRA_URL ]]; then
        osc8_link "$WORMHOLE_JIRA_URL" "$name"
    else
        echo -n "$name"
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
        echo -n "($(osc8_link "$url" "$branch"))"
    else
        echo -n "($branch)"
    fi
}

PROMPT='%(?:'                   # Introduce ternary expression using last exit status as condition
PROMPT+='%{$fg_bold[cyan]%}'    # Cyan for condition-true branch
PROMPT+='$(prompt_dir_display)' # Call function to get directory display
PROMPT+=':'                     # Ternary expression separator
PROMPT+='%{$fg[red]%}'          # Red for condition-false branch
PROMPT+='$(prompt_dir_display)' # Call function to get directory display
PROMPT+=')'                     # End ternary expression
PROMPT+='%{$reset_color%}'
PROMPT+='%{$fg[red]%}'
PROMPT+='$(prompt_git_branch)'
PROMPT+='%{$reset_color%} '
