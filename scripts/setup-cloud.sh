#!/usr/bin/env bash
# Richtet eine frische Cloud-Umgebung (Ubuntu hinter dem HTTPS-Proxy) für den
# App-Build ein: Systembibliotheken für Bevy und aktuelles Rust.
set -euo pipefail

if [[ -n "${HTTPS_PROXY:-}" ]]; then
    # Der Proxy lässt nur HTTPS durch; apt liest als eigener Benutzer, daher
    # das Zertifikat an eine lesbare Stelle kopieren.
    sed -i 's#http://\(archive\|security\).ubuntu.com#https://\1.ubuntu.com#' \
        /etc/apt/sources.list.d/*.sources /etc/apt/sources.list 2>/dev/null || true
    if [[ -f /root/.ccr/ca-bundle.crt ]]; then
        install -m 644 /root/.ccr/ca-bundle.crt /etc/apt/ccr-ca.crt
        ca='Acquire::https::CAInfo "/etc/apt/ccr-ca.crt";'
    fi
    printf 'Acquire::http::Proxy "%s";\nAcquire::https::Proxy "%s";\n%s\n' \
        "$HTTPS_PROXY" "$HTTPS_PROXY" "${ca:-}" > /etc/apt/apt.conf.d/99proxy
fi
apt-get update -qq || true
apt-get install -y -qq pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
rustup update stable
