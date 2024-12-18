<div align="center">
    <h1>
        🥤 straw 🍓🧃🍹🧋👒
    </h1>
    <h3>
        an opinionated take on stow with sync capabilities and optional config for per-platform overrides
    </h3>
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
per-platform configuration options and git integration.

## Installation

Currently the easies and best option is to use `cargo install`. If you don't have
the rust toolchain on your system, visit [Install Rust](https://www.rust-lang.org/tools/install).

``` bash
cargo install --git https://git.net.dalton.gd/dalton/straw.git
```

You can also clone this repo and compile it yourself.

## Usage

straw expects your configs to be in a stow-like directory structure under your
`~/.dotfiles` directory.

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

The config file can be placed in `~/.dotfiles/config.json`.

Here is the example config:

```json
{
    "git": {
        "remote": "origin",
        "branch": "main"
    },
    "overrides_windows": [
        [
            "powershell",
            {
                "from": "Microsoft.PowerShell_profile.ps1",
                "to": "%userprofile%/Documents/PowerShell/Microsoft.PowerShell_profile.ps1"
            }
        ],
        [
            "nvim",
            {
                "from": ".config/nvim",
                "to": "%localappdata%/nvim"
            }
        ],
        [
            "yazi",
            {
                "from": ".config/yazi",
                "to": "%appdata%/yazi/config"
            }
        ]
    ],
    "ignores_unix": [
        "komorebi",
        "whkd",
        "winfetch",
        "powershell"
    ],
    "ignores_windows": [
        "fish"
    ]
}
```

Available keys:

```json
"git"                 // git config overrides
"overrides"           // path overrides for all platforms
"overrides_windows"   // path overrides for windows
"overrides_unix"      // path overrides for unix
"ignores"             // ignored configs for all platforms
"ignores_windows"     // ignored configs for all windows
"ignores_unix"        // ignored configs for all unix
```

The `overrides` and `ignores` syntax follows the example above.

In the overrides:

- The `from` path is relative to the `.dotfiles`
- You can use environment variables in the `to` path
  - e.g. `%userprofile%` on windows
  - e.g. `$HOME` on unix
  - in the cross-platform overrides you can use either syntax, but make sure the
  environment variable is present on all systems you deploy your configs to

## Todos

- [ ] cache deployed symlinks and add `--clear` or a similar command
- [ ] env vars, also cached
- [ ] adopt ..?
- [ ] use profiles' directory structures when path folding
- [ ] swappable profiles
- [ ] per-device (hostname) ignores/overrides
- [ ] json is cumbersome to use and doesn't allow trailing commas. replace with toml? or lua?
- [x] resolve `~` in paths

- [ ] additional console args for overrides (especially useful for initial clone)

- [x] add `ssh2` with `openssl-on-win32` for windows to let `git2` handle cloning
over `ssh://`
- [ ] git merge flavor, remote and branch (repo config) override args
- [ ] git status
- [ ] git pull only
- [ ] git sync with a commit message
- [ ] git conflict resolution ..? (or post-conflict resolution sync option? maybe)
separate commit + push command could suffice?)
