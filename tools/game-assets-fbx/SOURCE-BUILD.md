# Rebuilding the experimental helper

This bundle includes the helper's C++ sources/build files in `source/helper`,
the full source trees at the pinned Assimp and ufbx commits in bare Git repositories,
and the reviewed Assimp patch. No dependency download is needed. The source
repositories contain only the pinned commit history boundary, not checkout-local
configuration, hooks or remote URLs. License notices are in `licenses` and in the
dependency source trees. The helper is part of gcad under the included GPL license.

Prerequisites: Git, a POSIX shell, `sha256sum`, CMake 3.24 or newer, and a C++17 compiler.
Windows validation uses Git Bash, Visual Studio's CMake and MSVC with the static
runtime. Set `CMAKE` to the executable path if it is not on PATH.

```sh
# Run from the bundle directory. The destination must not exist.
sha256sum --strict -c SHA256SUMS
sh rebuild-helper.sh /absolute/path/to/new-build
```

The script first checks the complete bundle manifest, then clones the included
repositories locally, applies and verifies the included patch,
and builds only the helper. The CMake identity check verifies the Assimp commit
and full tracked patch at configuration/build time. The output is
`new-build/build/Release/gcad_fbx_helper.exe` with Windows multi-configuration
generators, or `new-build/build/gcad_fbx_helper` with single-configuration generators.
Test it with `--handshake` and gcad's experimental conversion command.

The checked-out files and `source/helper` are the editable source form. After
intentional source edits, use CMake directly in the prepared build directory;
the rebuild script deliberately checks the original distribution manifest. To modify
the dependency, review and update the expected dependency identity as well; a
modified helper is not automatically a reviewed gcad converter. The repository's
Rust integration/engine tests are separate from this standalone native build.

Rebuilding establishes source/build completeness, not bit-for-bit binary
reproducibility, engine certification or clean-machine runtime acceptance.
