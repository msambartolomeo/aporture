# Aporture GTK

Gui client for aporture built with gtk4 and libadwaita using relm4

## Building

Clone the repository and have `Rust` and `cargo` installed

Install the dependencies:
```bash
# Fedora
sudo dnf install gtk4-devel libadwaita-devel gcc
```

Build Application:
```bash
SERVER_ADDRESS=<SERVER_ADDRESS> cargo build --release
```

## Flatpak

Requires `flatpak` and `flatpak-builder` to be installed, and `envsubst` to set server address to something other than localhost.

To build the app as flatpak run the following commands in the `../flatpak` folder

```bash
# Replace server address
SERVER_ADDRESS=<SERVER_ADDRESS> envsubst < dev.msambartolomeo.aporture.flatpak.json > flatpak.json

# Build and install
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo --install builddir flatpak.json

# Produce .flatpak to share
flatpak build-bundle repo aporture.flatpak dev.msambartolomeo.aporture --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo

# Install
flatpak install --user aporture.flatpak

# Run
flatpak run dev.msambartolomeo.aporture
```

