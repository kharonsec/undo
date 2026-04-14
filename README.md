# undo

Universal operation undo. Shadows file operations in `~/.undo/`.

## Features

- `undo`: Reverts the last file operation (rm, mv, cp).
- `undo <N>`: Reverts the last N operations.
- `undo rm`: Specifically recovers the last deleted file(s).
- `undo ls`: Shows recent operations history.
- `undo clear`: Purges all history and cached files.
- Shell integration: Wraps `rm`, `mv`, and `cp` to make them undoable.

## Installation

Run the provided `install.sh`:

```bash
./install.sh
```

## Shell Hooks

To enable undoable file operations, add the following to your `.bashrc` or `.zshrc`:

```bash
# undo hooks
rm() {
    undo record-rm "$@"
}

mv() {
    if [ "$#" -ne 2 ]; then
        command mv "$@"
    else
        undo record-mv "$1" "$2"
    fi
}

cp() {
    if [ "$#" -ne 2 ]; then
        command cp "$@"
    else
        undo record-cp "$1" "$2"
    fi
}
```

*Note: The wrappers for `mv` and `cp` are simplified to handle 2 arguments. For more complex use cases, the `command` prefix can be used to bypass the hook.*

## Usage Examples

```bash
# Delete a file
rm important.txt
# Oops! Undo it
undo

# Move a file
mv old.txt new.txt
# Undo the move
undo

# Delete multiple files and then recover them
rm *.log
undo rm

# See history
undo ls

# Undo last 3 operations
undo 3
```

## Storage

Metadata and shadowed files are stored in `~/.undo/`.
- `~/.undo/history.json`: Operations log.
- `~/.undo/trash/`: Cached files for recovery.
