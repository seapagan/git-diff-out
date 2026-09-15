#!/bin/sh
set -eu

root=$(mktemp -d)
trap 'rm -rf "$root"' 0
trap 'exit 1' 1 2 3 15

mkdir -p "$root/bin" "$root/wget-bin" "$root/archive"
for command in cat chmod cp grep gzip head install mkdir mktemp rm sed tar; do
    path=$(command -v "$command")
    ln -s "$path" "$root/bin/$command"
    ln -s "$path" "$root/wget-bin/$command"
done

cat > "$root/bin/uname" <<'EOF'
#!/bin/sh
if [ -z "${TEST_OS:-}" ]; then
    exec "$TEST_REAL_UNAME" "$@"
fi
case "$1" in
    -s) printf '%s\n' "$TEST_OS" ;;
    -m) printf '%s\n' "$TEST_ARCH" ;;
esac
EOF
cp "$root/bin/uname" "$root/wget-bin/uname"
chmod +x "$root/bin/uname" "$root/wget-bin/uname"

cat > "$root/bin/curl" <<'EOF'
#!/bin/sh
printf '%s\n' "$2" >> "$TEST_LOG"
case "$2" in
    */releases/latest) cp "$TEST_API" "$4" ;;
    */releases/download/*) cp "$TEST_ARCHIVE" "$4" ;;
    *) exit 1 ;;
esac
EOF
cat > "$root/wget-bin/wget" <<'EOF'
#!/bin/sh
printf '%s\n' "$3" >> "$TEST_LOG"
case "$3" in
    */releases/latest) cp "$TEST_API" "$2" ;;
    */releases/download/*) cp "$TEST_ARCHIVE" "$2" ;;
    *) exit 1 ;;
esac
EOF
chmod +x "$root/bin/curl" "$root/wget-bin/wget"

printf 'new gd\n' > "$root/archive/gd"
printf 'new git-diff-out\n' > "$root/archive/git-diff-out"
tar -czf "$root/release.tar.gz" -C "$root/archive" gd git-diff-out
printf '{"tag_name": "0.2.0"}\n' > "$root/latest.json"

TEST_REAL_UNAME=$(command -v uname)
TEST_ARCHIVE=$root/release.tar.gz
TEST_API=$root/latest.json
TEST_LOG=$root/downloads.log
export TEST_REAL_UNAME TEST_ARCHIVE TEST_API TEST_LOG

fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

run_install() {
    : > "$TEST_LOG"
    env PATH="$TEST_BIN" /bin/sh ./install.sh > "$root/output" 2>&1
}

assert_asset() {
    expected="git-diff-out-v${GD_VERSION:-0.2.0}-$1.tar.gz"
    grep -q "/releases/download/${GD_VERSION:-0.2.0}/$expected" "$TEST_LOG" || fail "wrong asset: $expected"
}

TEST_OS=Linux TEST_ARCH=x86_64 GD_VERSION=0.1.0 GD_INSTALL_DIR="$root/install"
TEST_BIN=$root/bin
export TEST_OS TEST_ARCH GD_VERSION GD_INSTALL_DIR
mkdir -p "$GD_INSTALL_DIR"
printf 'old gd\n' > "$GD_INSTALL_DIR/gd"
printf 'old git-diff-out\n' > "$GD_INSTALL_DIR/git-diff-out"
run_install
assert_asset x86_64-unknown-linux-gnu
! grep -q '/releases/latest' "$TEST_LOG" || fail 'GD_VERSION fetched latest'
grep -q 'new gd' "$GD_INSTALL_DIR/gd" || fail 'gd was not replaced'
grep -q 'new git-diff-out' "$GD_INSTALL_DIR/git-diff-out" || fail 'git-diff-out was not replaced'
test -x "$GD_INSTALL_DIR/gd" && test -x "$GD_INSTALL_DIR/git-diff-out" || fail 'binaries are not executable'
grep -q 'add it to PATH' "$root/output" || fail 'missing PATH warning'

GD_VERSION=v0.1.0
run_install
assert_asset x86_64-unknown-linux-gnu

TEST_OS=Darwin TEST_ARCH=x86_64 GD_VERSION=''
run_install
assert_asset x86_64-apple-darwin
grep -q '/releases/latest' "$TEST_LOG" || fail 'latest release was not fetched'

TEST_ARCH=arm64
run_install
assert_asset aarch64-apple-darwin

TEST_OS=Linux TEST_ARCH=aarch64
if run_install; then fail 'unsupported Linux ARM succeeded'; fi
grep -q 'unsupported' "$root/output" || fail 'unsupported target message missing'

TEST_OS=Darwin TEST_ARCH=x86_64 GD_VERSION=0.1.0
TEST_BIN=$root/wget-bin
run_install
assert_asset x86_64-apple-darwin

TEST_BIN=$root/bin
GD_INSTALL_DIR="$root/new bin"
run_install
test -x "$GD_INSTALL_DIR/gd" && test -x "$GD_INSTALL_DIR/git-diff-out" || fail 'install directory was not created'
TEST_BIN="$root/bin:$GD_INSTALL_DIR"
run_install
! grep -q 'add it to PATH' "$root/output" || fail 'PATH warning appeared for an existing entry'

GD_INSTALL_DIR=''
XDG_BIN_HOME="$root/xdg bin"
export XDG_BIN_HOME
TEST_BIN=$root/bin
run_install
test -x "$XDG_BIN_HOME/gd" || fail 'XDG_BIN_HOME default was not used'
XDG_BIN_HOME=''
HOME="$root/home"
export HOME
run_install
test -x "$HOME/.local/bin/gd" || fail 'HOME default was not used'

TEST_OS='' TEST_ARCH='' GD_VERSION=0.1.0
TEST_BIN=$root/bin
if [ -n "${EXPECTED_INSTALL_TARGET:-}" ]; then
    run_install
    assert_asset "$EXPECTED_INSTALL_TARGET"
fi

printf 'Unix installer tests passed\n'
