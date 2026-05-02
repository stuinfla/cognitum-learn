# Building learn-rs

This workspace depends on RuVector crates via a `../ruvector` sibling directory.
Cargo path deps are relative: `../../../ruvector/crates/<name>` (from inside `crates/<name>/`).

**Stuart's machine:** the symlink is already present at
`/Users/stuartkerr/Code/Video watcher skill/ruvector -> ~/RuVector_Clean`.

**CI (GitHub Actions):** the release workflow does a shallow clone of the public
repo `ruvnet/RuVector` into the sibling path `../ruvector` before any Cargo
invocation. No secrets are required — the repo is MIT-licensed and public.

```yaml
- name: Clone ruvnet/RuVector (sibling, public)
  shell: bash
  run: |
    git clone --depth 1 --branch main \
      https://github.com/ruvnet/RuVector.git \
      ../ruvector
```

**Other machines:** clone or symlink `ruvnet/RuVector` (or `~/RuVector_Clean`) to
the sibling path before running `cargo check`:
```
git clone --depth 1 https://github.com/ruvnet/RuVector.git ../ruvector
```
The directory must be a sibling of `learn-rs/`, not inside it.
