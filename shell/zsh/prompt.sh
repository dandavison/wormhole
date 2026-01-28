# using __git_ps1 from https://github.com/git/git/blob/master/contrib/completion/git-prompt.sh
setopt prompt_subst

export GIT_PS1_SHOWDIRTYSTATE=yes
export GIT_PS1_UNSTAGED="અ "
export GIT_PS1_STAGED="જ "

function prompt_dir_display {
    if [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD != $WORMHOLE_PROJECT_DIR ]]; then
        echo -n "${WORMHOLE_PROJECT_NAME}/$(realpath --relative-to="$WORMHOLE_PROJECT_DIR" "$PWD")"
    elif [[ -n $WORMHOLE_PROJECT_DIR ]] && [[ $PWD == $WORMHOLE_PROJECT_DIR ]]; then
        echo -n "${WORMHOLE_PROJECT_NAME}"
    else
        echo -n "${PWD/#$HOME/~}"
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
PROMPT+='$(__git_ps1 "(%s)")'
PROMPT+='%{$reset_color%} '
