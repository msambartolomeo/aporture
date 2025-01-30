export RUST_LOG := "aporture=debug"

all: linux windows flatpak

release_dir:
    mkdir -p ./release

linux: release_dir
    cargo build --release
    cp ./target/release/aporture ./target/release/aporture_server ./target/release/aporture_gtk ./release/

flatpak: release_dir
    cd ./flatpak && flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo --install builddir dev.msambartolomeo.aporture.flatpak.json
    cd ./flatpak && flatpak build-bundle repo aporture.flatpak dev.msambartolomeo.aporture --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo
    mv ./flatpak/aporture.flatpak ./release/

windows: release_dir
    podman run --rm -it -v .:/mnt:z powerball253/gtk4-cross:rust-gtk-4.14 bash -c "build && package"
    mv ./package/aporture.exe ./package/aporture_server.exe ./release/
    zip ./release/Aporture.zip ./package
    rm -rf ./package

