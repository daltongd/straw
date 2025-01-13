<div align="center">
    <h1>
        🥤 🍓 🧃 straw 🍹 🧋 👒
    </h1>
    <p>
        <strong>
        an opinionated take on <a href="https://www.gnu.org/software/stow/">GNU Stow</a> with <a href="https://git-scm.com/">git</a>-based sync capabilities with an optional <a href="https://www.lua.org/">lua</a>-based config
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
`~/.dotfiles` directory. Directories within `~/.dotfiles` with names that begin with a dot (`.`, e.g. -> `.straw`) are ignored and not treated as packages.

To clone your dotfiles repo to this path, use:

> [!NOTE]
> If you already have your .dotfiles there, there's no need to clone. Make sure
> you have your git repository initialized and that the `origin` remote url is set.

```bash
straw clone https://repository.com/your/.dotfiles.git --apply
```

To later sync your dotfiles:

```bash
straw --sync --apply
```

or a shorthand version:

```bash
straw -sa
```

You can also use the `--preview` option to only log out the changes that would be
applied without applying them (note, the sync command won't happen, it'll only
check if there are any changes):

```bash
straw -sa --preview
```

or a shorthand version:

```bash
straw -sap
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
  from = "Microsoft.PowerShell_profile.ps1",
  to = "%userprofile%/Documents/PowerShell/Microsoft.PowerShell_profile.ps1",
 }
 config.overrides["nvim"] = { from = ".config/nvim", to = "%localappdata%/nvim" }
 config.overrides["yazi"] = { from = ".config/yazi", to = "%appdata%/yazi/config" }
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

## Todos

- [ ] additional option for dotfiles handling by replacing `dot-` with a period `.`
as in GNU Stow
- [ ] lua callbacks (e.g. for applying a package/packages, etc.)
- [ ] configurable one-time deploy by copying the file directly

- [ ] simple test suite

Nice to haves:

- [ ] installing packages / running scripts (this could also solve the env vars)
- [ ] setting the env vars, also cached (might not be feasible, on all platforms,
also could be set by the shell profile for the most part)

- [ ] plugin manager
- [ ] git lua bindings

Done:

- [~] swappable profiles (via the `.dotfiles` path override)

- [x] patches - ability to get a specific directory/file from any dotfiles repository
- [x] utility functions in lua library
- [x] git ssh key selection

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
