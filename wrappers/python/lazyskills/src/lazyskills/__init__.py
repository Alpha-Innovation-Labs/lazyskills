from __future__ import annotations

import os
import platform
from pathlib import Path
import shutil
import sys
import urllib.request
from importlib.metadata import PackageNotFoundError, version as package_version


try:
    VERSION = package_version("lazyskills")
except PackageNotFoundError:
    VERSION = "0.1.1"

RELEASE_BASE_URL = os.environ.get(
    "LAZYSKILLS_RELEASE_BASE_URL",
    f"https://github.com/Alpha-Innovation-Labs/lazyskills/releases/download/v{VERSION}",
)


def _same_file(first: Path, second: Path) -> bool:
    try:
        return first.resolve() == second.resolve()
    except OSError:
        return False


def _find_installed_binary() -> Path | None:
    script_path = Path(sys.argv[0])
    cargo_binary = Path.home() / ".cargo" / "bin" / "lazyskills"
    if (
        cargo_binary.exists()
        and cargo_binary.is_file()
        and os.access(cargo_binary, os.X_OK)
    ):
        if not _same_file(cargo_binary, script_path):
            return cargo_binary

    located = shutil.which("lazyskills")
    if not located:
        return None

    located_path = Path(located)
    if _same_file(located_path, script_path):
        return None

    return located_path


def _resolve_target() -> tuple[str, str]:
    system = platform.system().lower()
    machine = platform.machine().lower()

    targets = {
        ("darwin", "arm64"): ("aarch64-apple-darwin", "lazyskills"),
        ("linux", "x86_64"): ("x86_64-unknown-linux-gnu", "lazyskills"),
        ("windows", "amd64"): ("x86_64-pc-windows-msvc", "lazyskills.exe"),
    }

    target = targets.get((system, machine))
    if target is None:
        raise RuntimeError(f"Unsupported platform: {system}/{machine}")
    return target


def _cache_root() -> Path:
    if os.name == "nt":
        local = os.environ.get("LOCALAPPDATA")
        if local:
            return Path(local)
        return Path.home() / "AppData" / "Local"

    xdg = os.environ.get("XDG_CACHE_HOME")
    if xdg:
        return Path(xdg)
    return Path.home() / ".cache"


def _download(url: str, destination: Path) -> None:
    with urllib.request.urlopen(url) as response:
        if response.status != 200:
            raise RuntimeError(f"Download failed ({response.status}) for {url}")
        destination.write_bytes(response.read())


def _ensure_managed_binary() -> Path:
    target_triple, binary_name = _resolve_target()

    install_dir = _cache_root() / "lazyskills" / "bin" / VERSION / target_triple
    install_dir.mkdir(parents=True, exist_ok=True)
    binary_path = install_dir / binary_name

    if binary_path.exists() and os.access(binary_path, os.X_OK):
        return binary_path

    asset_name = (
        f"lazyskills-{target_triple}{'.exe' if binary_name.endswith('.exe') else ''}"
    )
    asset_url = f"{RELEASE_BASE_URL}/{asset_name}"

    print(
        f"`lazyskills` binary not found in PATH. Downloading {VERSION} for {target_triple}...",
        file=sys.stderr,
    )
    _download(asset_url, binary_path)

    if os.name != "nt":
        binary_path.chmod(0o755)

    return binary_path


def main() -> None:
    args = ["lazyskills", *sys.argv[1:]]
    binary = _find_installed_binary()

    if binary is None:
        binary = _ensure_managed_binary()

    os.execv(binary, args)
