# Aporture GTK

Gui client for aporture built with gtk4 and libadwaita using relm4

## Building

Clone the repository and have `Rust` and `cargo` installed

### Dependencies

```bash
# Fedora
sudo dnf install gtk4-devel libadwaita-devel gcc
```

## Flatpak

Requires `flatpak` and `flatpak-builder` to be installed

To build the app as flatpak run the following commands in the `../flatpak` folder

```bash
# Build and install
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo --install builddir dev.msambartolomeo.aporture.flatpak.json

# Produce .flatpak to share
flatpak build-bundle repo aporture.flatpak dev.msambartolomeo.aporture --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo

# Install
flatpak install --user aporture.flatpak

# Run
flatpak run dev.msambartolomeo.aporture
```

