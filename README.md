# slim-addons

The official module registry for [slim-m](https://github.com/NC1107/slim-m).

A slim-m space installs modules from here through the Dock (space settings, admin only).
slim-m ships the module *system*; each module here ships its own behavior. See
slim-m's `docs/decisions/0021-modules-and-the-dock.md`.

## Layout

- `index.json` - the list the Dock browses.
- `modules/<id>/manifest.json` - one module's contract (permissions it adds, host
  capabilities it asks for, runtime backend it needs, and its artifact + sha256).
- `modules/<id>/<version>/module.wasm` - the built module artifact, pinned by version.

Nothing here runs until an admin installs it into a space, and then only with the
permissions and capabilities the admin approves.

## Building your own module

Start from [`modules/_template/`](modules/_template/) - a copyable starter that
builds as-is, with a text command, a slash command, and a launchable interactive
app. Its README walks through making it yours, and
[`scripts/package-module.sh`](scripts/package-module.sh) builds the wasm, places
it at its versioned path, and pins its SHA-256 into the manifest.

The full contract - the ABI, the manifest, every extension-point kind, the scene
contract, limits, and publishing - is in slim-m's
[`docs/modules/building-modules.md`](https://github.com/NC1107/slim-m/blob/main/docs/modules/building-modules.md).

## Running a module before you install it

`tools/run-module` drives a module exactly the way slim does - same ABI, same
wasmi version, and the same refusal of a module that imports anything - so you
can see what yours actually emits without installing it into a space.

```bash
cargo build --release --manifest-path tools/run-module/Cargo.toml
R=tools/run-module/target/release/run-module

# a command, with its input
$R modules/spirograph/0.1.0/module.wasm spiro ''

# an interactive frame, the way a tap arrives
$R modules/spirograph/0.1.0/module.wasm spiro '{"action":"random","state":"34,13,21"}'
```

It does not enforce slim's fuel, memory or wall-clock limits, so a module that
runs here can still be refused there for being too expensive. The manifest's
`runtime.limits` is the authority on that.

### Checking a scene against the renderer

If your module draws, `scripts/check-scene-ops.py` reads its output the way the
client's parser does and reports anything that would be silently dropped - an op
that does not exist, a key on the wrong op, a colour that is not a scene colour.
Pass it the runner, the wasm, the command, and as many inputs as you want
covered:

```bash
python3 scripts/check-scene-ops.py "$R" modules/spirograph/0.1.0/module.wasm spiro \
  '' '{"action":"random","state":"34,13,21"}' 'not json at all'
```

This is worth running on malformed input too. A scene op the client drops does
not fail loudly anywhere - it just does not draw - which is how three of these
shipped before the check existed.
