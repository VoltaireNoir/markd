# markd
Bookmark directories for easy directory-hopping in the terminal.

![](https://github.com/VoltaireNoir/markd/blob/033206124d8c0108e541c3a28ed36fa586e9fcf5/screen1.png?raw=true)
![](https://github.com/VoltaireNoir/markd/blob/033206124d8c0108e541c3a28ed36fa586e9fcf5/screen2.png?raw=true)

All it takes is one command `markd` to bookmark your current directory, or use the `-p / --path` to specify custom path and `-a / --alias` to use a set a custom bookmark name. All paths are ensured to be valid ones, relative paths are stored in their expanded forms and names are always lowercase. No duplicate names are allowed (use an alias).
The CLI tool also provides necessary functionality to remove, search and clean your directory bookmarks. For example, the `purge` command will check all the paths and remove the ones that no longer exist. The `list` command supports `--filter`, `--start` and `--end` for advanced searching.

Run `markd help` for a full list of supported commands and argumnets. Run `markd <COMMAND> --help` to get more info on the command.

Note: bookmarks are stored in `bookmarks.json` file in the user home directory in the form of `"name":"path"`, which can also be directly edited if necessary.

## Shell Support
Since 'cd' is a built-in shell command, you need to use 'command substitution' to make use of markd to switch directories.
To make it work, simply add a function definition to your shell config file.
> Note: The function name used here is 'goto' but you can change it to whatever you prefer.

### Fish
- Create functions directory in fish config folder (usually `/home/user/.config/fish`)
- Inside the folder, create a file named `goto.fish`
- Copy paste the following code and save it
    ```
      function goto
        cd $(markd g $argv)
      end
    ```
### Zsh and Bash
- Add the following code to your `.zshrc` or `.bashrc`
    ```
    goto() {
      cd $(markd g $1);
    }
    ```
### Powershell (untested)
- Open powershell and open your config file by running `notepad $profile`
- Add the following code to and save it
    ```
    function goto([string]$Bookmark) {
      cd (markd g $Bookmark)
    }
    ```
---
After adding the ncessary code to your shell config, you should be able to jump between directories using the command `goto <bookmark-name>`.
