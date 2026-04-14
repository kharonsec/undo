#!/bin/bash
set -e

echo "Building undo..."
cargo build --release

echo "Installing undo..."
sudo cp target/release/undo /usr/local/bin/

echo "Success! Add the following to your .bashrc or .zshrc to enable hooks:"
echo '
rm() {
    undo record-rm "$@"
}
mv() {
    if [ "$#" -ne 2 ]; then command mv "$@"; else undo record-mv "$1" "$2"; fi
}
cp() {
    if [ "$#" -ne 2 ]; then command cp "$@"; else undo record-cp "$1" "$2"; fi
}
'
