# Version 0.3.0

- Plain text output support: `markd list --plain`
- Failsafe mode for `get` command using the `--failsafe` flag
  - Writes current working dir path to stdout on error
- Improved `goto` shell function/command with optional `fzf` support
  - Invoke `goto` without arguments to fuzzy search through list of bookmarks (requires `fzf`)
  - Fixed `goto` switching to home dir on error (when no entry is found)

# Version 0.2.1

- Release unrelated to the program itself. Pushed to fix cargo-dist and generate binaries.

# Version 0.2.0

- Added `shell` command to generate specified shell functions to make 'goto' command work
- Improved table formatting when listing bookmarks
- markd now uses `TOML` format instead of `JSON` to store bookmarks (`markd migrate` command is provided to help with migration)

# Version 0.1.1

- Added the `clip` command to bookmark directories to a temporary name-space, which is accessed by default when `markd get` is run without any additional arguments
