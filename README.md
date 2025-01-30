<div align="center">
    <h1>
        🥤 🍓 🧃 straw 🍹 🧋 👒
    </h1>
    <p>
        <strong>
            an opinionated take on
            <a href="https://www.gnu.org/software/stow/">GNU Stow</a>
            with <a href="https://git-scm.com/">git</a>-based
            sync capabilities with an optional
            <a href="https://www.lua.org/">lua</a>-based config
        </strong>
    </p>
    <hr />
    <!-- <a href="https://github.com/daltongd/yanklock.nvim/actions/workflows/run_tests.yml"><img src="https://img.shields.io/github/actions/workflow/status/daltongd/yanklock.nvim/run_tests.yml?style=for-the-badge&logo=github&label=tests&labelColor=%23aa55aa"></a> -->
    <!-- <a href="https://github.com/daltongd/yanklock.nvim?tab=MIT-1-ov-file#readme"><img src="https://img.shields.io/github/license/daltongd/yanklock.nvim?style=for-the-badge&logo=opensourceinitiative"></a> -->
</div>

> [!WARNING]
> This tool is a work in progress and is developed as my personal utility. I may
> or may not update it further and add new features. I'm also not giving any
> guarantees that your configs are safe with this tool, it's highly experimental
> and it's in the experiments phase. This tool is offered as-is and I will not be
> responding to feature requests that aren't aligned with my needs.

> [!NOTE]
> Contributions are always welcome, but due to the experimental/wip nature of this
> project, I may be slow to react to issues / pull requests until further notice.

## What is straw?

An opinionated and pretty basic dotfiles manager inspired by stow, yet with additional
configuration options and a built-in git integration.

## Installation

Currently the easies and best option is to use `cargo install`. If you don't have
the rust toolchain on your system, visit [Install Rust](https://www.rust-lang.org/tools/install).

``` bash
cargo install --git https://git.net.dalton.gd/dalton/straw.git
```

You can also clone this repo and compile it yourself.

## Usage

straw expects your configs to be in a stow-like directory structure under your
`~/.dotfiles` directory. Directories within `~/.dotfiles` with names that begin with
a dot (`.`, e.g. -> `.straw`) are ignored and not treated as packages.

To clone your dotfiles repo to this path, use:

> [!NOTE]
> If you already have your .dotfiles there, there's no need to clone. Make sure
> you have your git repository initialized and that the `origin` remote url is set.

```bash
straw clone https://repository.com/your/.dotfiles.git --apply
```

To later sync your dotfiles:

```bash
straw sync
```

or:

```bash
straw -sa
```

which stands for:

```bash
straw sync --sync-repo --apply
```

You can also use the `--preview` option to only log out the changes that would be
applied without applying them (note, the sync command won't happen, it'll only
check if there are any changes):

```bash
straw sync --preview
```

or a shorthand version:

```bash
straw sync -p
```

## Configuration

straw uses a lua-based config inspired by the likes of [WezTerm](https://wezfurlong.org/wezterm)
or [Neovim](https://neovim.io/)
The config file can be placed in `~/.dotfiles/.straw/config.lua`.

Here is an example config:

```lua
local straw = require("straw")
local config = {}

config.git = {
    remote = "origin",
    branch = "main",
}

config.overrides = {}

if straw.platform == "windows" then
    config.overrides["powershell"] = {
        link = "%userprofile%/Documents/PowerShell/Microsoft.PowerShell_profile.ps1",
        target = "Microsoft.PowerShell_profile.ps1",
    }
    config.overrides["nvim"] = {
        link = "%localappdata%/nvim"
        target = ".config/nvim",
    }
    config.overrides["yazi"] = {
        link = "%appdata%/yazi/config"
        target = ".config/yazi",
    }
end

if straw.platform == "windows" then
    config.ignores = { "fish" }
elseif straw.platform == "unix" then
    config.ignores = { "komorebi", "whkd", "winfetch", "powershell", "wpm" }
end

return config
```

In the overrides:

- The `from` path is relative to the `.dotfiles`
- You can use environment variables in the `to` path
  - e.g. `%userprofile%` on windows
  - e.g. `$HOME` on unix
  - you can also us `~` as a home directory on all platforms

## Help

```txt
❯ straw --help
Usage: straw.exe [OPTIONS] [COMMAND]

Commands:
  clone       Clone a git repository into ~/.dotfiles
  init        Init a git repository in ~/.dotfiles, you can also provide the origin url
  status      Get git status
  sync        Synchronize the .dotfiles (commit, push, pull, merge/rebase) and apply the packages according to the config. Equivalend of `--sync-repo --apply` or `-sa`
  clear       Clear packages
  copy-files  Copy the specified package(s) over instead of creating symlinks. Useful in combination with ignores if you'd like to only copy the initial config and not manage with straw
  adopt       Adopt the package by moving the contents of a directory to the `.dotfiles` path and creating a symlink NOTE: This functionality is limited to the paths under your target directory
  patch       Patch your dotfiles with a file or a directory from a remote repository
  run         Run lua module
  execute     Execute lua script
  help        Print this message or the help of the given subcommand(s)

Options:
  -p, --preview                Preview the operation. Prints the messages, but doesn't perform any operations
  -s, --sync-repo              Synchronize the .dotfiles with a git repository. Performs Commit -> Pull -> Rebase/Merge -> Push
  -a, --apply                  Apply the packages onto the system
  -c, --clear                  Clear all deployed symlinks
      --packages <PACKAGE>...  Specify packages to apply. Implies `--apply`, overrides ignores from the config
      --ignore <PACKAGE>...    Specify packages to ignore when applying the config
      --remote <REMOTE>        Override git remote setting for this operation
      --branch <BRANCH>        Override git branch setting for this operation
      --merge-commit           Set the merge flavor to a merge commit
      --rebase                 Set the merge flavor to rebase
  -m, --message <MESSAGE>      Set the commit message. Implies `--commit`
      --pull                   Pull .dotfiles from the repository. This selectively enables the `pull` action, even if the `--sync-repo` flag is not set
      --push                   Push .dotfiles to the repository. This selectively enables the `push` action, even if the `--sync-repo` flag is not set
      --commit                 Commit .dotfiles to the repository. This selectively enables the `commit` action, even if the `--sync-repo` flag is not set
      --adopt <PACKAGE>        Adopt the package by moving the contents of a directory to the `.dotfiles` path and creating a symlink. NOTE: This functionality is limited to the paths under your target directory
      --dotfiles <PATH>        Override the default `~/.dotfiles` path
      --target <PATH>          Override the target path (default: `~`)
      --ssh-key-path <PATH>    Direct path to the ssh key (by default looks for a standard-named key in the `~/.ssh/` dir)
  -h, --help                   Print help
  -V, --version                Print version
```

## Todos

Pre-release checklist:

- [ ] Document features

Features:

- [ ] lua hooks (callbacks, e.g. for applying a package/packages, etc.)
- [ ] simple test suite

Nice to haves:

- [ ] plugin manager

On Hold:

- [!] additional option for dotfiles handling by replacing `dot-` with a period `.`
as in GNU Stow. this one is tricky, because this can only work up until some level
in the directory structure

- [!] setting the env vars, also cached for ease of reverting them later
(might not be feasible, on all platforms, also could be set by the shell
profile for the most part)

Done:

- [~] git lua bindings (unstable)
- [~] swappable profiles (via the `.dotfiles` path override)

- [x] undo/clear
- [x] configurable one-time deploy by copying the file directly
- [x] patches - ability to get a specific directory/file from any dotfiles repository
- [x] utility functions in lua library
- [x] git ssh key selection
- [x] installing packages / running scripts

- [x] `.dotfiles` path override
- [x] `target` path override
- [x] cache deployed symlinks and add `--clear` or a similar command
- [x] use packages' directory structures when path folding
- [x] per-device (hostname) ignores/overrides - instead add hostname to lua module
- [x] per-username ignores/overrides - instead add username to lua module
- [x] replace json with lua for some added pizzaz
- [x] resolve `~` in paths
- [x] adopt

- [x] additional console args for overrides (especially useful for initial clone)
things like: ignores, or ability to specify packages to apply

- [x] add `ssh2` with `openssl-on-win32` for windows to let `git2` handle cloning
over `ssh://`
- [x] git merge flavor, remote and branch (repo config) override args
- [x] git - determine if there are any changes to push/commit
- [x] git status
- [x] git pull only
- [x] git push only
- [x] git sync with a commit message
- [x] git set the default commit message in the config
- [x] git init?
