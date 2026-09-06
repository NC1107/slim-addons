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
