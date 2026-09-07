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
