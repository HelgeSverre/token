# Fish Shell Syntax Highlighting Test
# A development environment setup and project management toolkit.

# ============================================================
# Variables and configuration
# ============================================================

set -gx EDITOR "token"
set -gx VISUAL $EDITOR
set -gx PAGER "less -R"
set -gx LANG "en_US.UTF-8"
set -gx LC_ALL $LANG

set -g fish_greeting ""  # Disable greeting

# XDG directories
set -gx XDG_CONFIG_HOME $HOME/.config
set -gx XDG_DATA_HOME $HOME/.local/share
set -gx XDG_CACHE_HOME $HOME/.cache

# Path management
fish_add_path $HOME/.local/bin
fish_add_path $HOME/.cargo/bin
fish_add_path /opt/homebrew/bin
fish_add_path $HOME/go/bin
fish_add_path $HOME/.bun/bin

# ============================================================
# Theme and prompt
# ============================================================

set -g fish_color_command green
set -g fish_color_param normal
set -g fish_color_error red --bold
set -g fish_color_comment brblack
set -g fish_color_quote yellow
set -g fish_color_operator cyan
set -g fish_color_escape cyan --bold
set -g fish_color_autosuggestion brblack

function fish_prompt
    set -l last_status $status
    set -l cwd (prompt_pwd)

    # Git info
    set -l git_branch ""
    set -l git_dirty ""
    if command -sq git; and git rev-parse --is-inside-work-tree &>/dev/null
        set git_branch (git branch --show-current 2>/dev/null)
        if test -n (git status --porcelain 2>/dev/null)
            set git_dirty "*"
        end
    end

    # Status indicator
    if test $last_status -ne 0
        set_color red
        printf "✗ "
    end

    # User@host (only in SSH)
    if set -q SSH_CONNECTION
        set_color brblack
        printf "%s@%s " (whoami) (hostname -s)
    end

    # Directory
    set_color blue --bold
    printf "%s" $cwd

    # Git branch
    if test -n "$git_branch"
        set_color brblack
        printf " on "
        set_color magenta
        printf "%s%s" $git_branch $git_dirty
    end

    # Prompt char
    set_color normal
    printf "\n❯ "
end

function fish_right_prompt
    set -l duration $CMD_DURATION
    if test $duration -gt 1000
        set_color brblack
        printf "%s" (math "scale=1; $duration / 1000")"s"
        set_color normal
    end
end

# ============================================================
# Abbreviations (expand on space)
# ============================================================

abbr -a g git
abbr -a ga "git add"
abbr -a gc "git commit"
abbr -a gco "git checkout"
abbr -a gd "git diff"
abbr -a gl "git log --oneline -20"
abbr -a gp "git push"
abbr -a gpl "git pull --rebase"
abbr -a gs "git status -sb"
abbr -a gsw "git switch"
abbr -a gwt "git worktree"

abbr -a c cargo
abbr -a cb "cargo build"
abbr -a cr "cargo run"
abbr -a ct "cargo test"
abbr -a cw "cargo watch -x run"

abbr -a k kubectl
abbr -a dc docker compose
abbr -a tf terraform

abbr -a ll "ls -la"
abbr -a la "ls -a"
abbr -a .. "cd .."
abbr -a ... "cd ../.."

# ============================================================
# Functions
# ============================================================

function mkcd --description "Create directory and cd into it"
    mkdir -p $argv[1]; and cd $argv[1]
end

function extract --description "Extract any archive format"
    if not test -f $argv[1]
        echo "Error: '$argv[1]' is not a file" >&2
        return 1
    end

    switch $argv[1]
        case '*.tar.gz' '*.tgz'
            tar xzf $argv[1]
        case '*.tar.bz2' '*.tbz2'
            tar xjf $argv[1]
        case '*.tar.xz' '*.txz'
            tar xJf $argv[1]
        case '*.tar.zst'
            tar --zstd -xf $argv[1]
        case '*.tar'
            tar xf $argv[1]
        case '*.zip'
            unzip $argv[1]
        case '*.gz'
            gunzip $argv[1]
        case '*.bz2'
            bunzip2 $argv[1]
        case '*.7z'
            7z x $argv[1]
        case '*.rar'
            unrar x $argv[1]
        case '*'
            echo "Unknown archive format: $argv[1]" >&2
            return 1
    end
end

function fzf-history --description "Search command history with fzf"
    history | fzf --no-sort --query (commandline) | read -l cmd
    if test -n "$cmd"
        commandline -r $cmd
    end
    commandline -f repaint
end

function ports --description "Show listening ports"
    if test (uname) = Darwin
        lsof -iTCP -sTCP:LISTEN -n -P | awk 'NR>1 {print $9, $1, $2}' | column -t
    else
        ss -tlnp | tail -n +2
    end
end

function weather --description "Show weather for a city"
    set -l city (string join '+' $argv)
    if test -z "$city"
        set city "Oslo"
    end
    curl -s "wttr.in/$city?format=3"
end

# ============================================================
# Project management
# ============================================================

function proj --description "Project management commands"
    set -l cmd $argv[1]
    set -e argv[1]

    switch $cmd
        case list ls
            # Find all git repos in common locations
            for dir in ~/code ~/projects ~/work
                if test -d $dir
                    find $dir -maxdepth 2 -name .git -type d 2>/dev/null | while read gitdir
                        set -l project (dirname $gitdir)
                        set -l name (basename $project)
                        set -l branch (git -C $project branch --show-current 2>/dev/null)
                        set -l dirty ""
                        if test -n (git -C $project status --porcelain 2>/dev/null)
                            set dirty " *"
                        end
                        printf "%-25s %-15s %s%s\n" $name $branch $project $dirty
                    end
                end
            end

        case open go cd
            set -l target $argv[1]
            for dir in ~/code ~/projects ~/work
                set -l path $dir/$target
                if test -d $path
                    cd $path
                    echo "→ $path"
                    return 0
                end
            end
            echo "Project not found: $target" >&2
            return 1

        case new create
            set -l name $argv[1]
            set -l template $argv[2]
            set -l dest ~/code/$name

            if test -d $dest
                echo "Project already exists: $dest" >&2
                return 1
            end

            mkdir -p $dest
            cd $dest
            git init

            switch $template
                case rust
                    cargo init .
                case node
                    npm init -y
                    echo "node_modules/" > .gitignore
                case python py
                    python3 -m venv .venv
                    echo ".venv/" > .gitignore
                    echo "__pycache__/" >> .gitignore
                case '*'
                    touch README.md
                    echo "# $name" > README.md
            end

            git add -A
            git commit -m "Initial commit"
            echo "Created project: $dest"

        case stats
            if not git rev-parse --is-inside-work-tree &>/dev/null
                echo "Not in a git repository" >&2
                return 1
            end
            echo "Commits: "(git rev-list --count HEAD)
            echo "Authors: "(git shortlog -sn --no-merges | wc -l | string trim)
            echo "Files:   "(git ls-files | wc -l | string trim)
            echo "Size:    "(du -sh .git | cut -f1)" (.git)"
            echo ""
            echo "Top contributors:"
            git shortlog -sn --no-merges | head -5

        case '*'
            echo "Usage: proj <list|open|new|stats> [args]"
            echo ""
            echo "Commands:"
            echo "  list           List all projects"
            echo "  open <name>    Open a project"
            echo "  new <name>     Create a new project"
            echo "  stats          Show repo statistics"
    end
end

# Tab completions for proj
complete -c proj -n "__fish_use_subcommand" -a "list open new stats" -f
complete -c proj -n "__fish_seen_subcommand_from open go cd" -a "(proj list 2>/dev/null | awk '{print \$1}')" -f
complete -c proj -n "__fish_seen_subcommand_from new create" -x -a "rust node python"

# ============================================================
# Keybindings
# ============================================================

bind \cr fzf-history
bind \cf 'set -l f (fzf --preview "head -50 {}"); and commandline -i $f'
bind \ce 'commandline -i (dirs | fzf)'

# ============================================================
# Startup
# ============================================================

# Load local overrides
if test -f ~/.config/fish/local.fish
    source ~/.config/fish/local.fish
end

# Auto-activate virtualenvs
function __auto_venv --on-variable PWD
    if test -d .venv; and not set -q VIRTUAL_ENV
        source .venv/bin/activate.fish
        echo "Activated virtualenv: .venv"
    end
end
