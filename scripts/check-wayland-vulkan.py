"""Validate Vulkan handoff and sustained workloads on an isolated Weston compositor.

Uses a private socket/runtime directory and stops only its own compositor.
The headless Pixman host and Lavapipe commonly used in CI do not prove physical
GPU or display scanout behavior. Use --backend=x11 for a visible nested session.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", choices=("headless", "x11"), default="headless")
    parser.add_argument("--output", type=Path, default=Path("target/vulkan-wayland"))
    parser.add_argument("--weston-root", type=Path,
                        help="optional extracted Debian package root containing usr/bin/weston")
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("this runner requires Linux and Weston")
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="run-", dir=args.output)).resolve()
    scripts = Path(__file__).resolve().parent
    env = dict(os.environ)
    # Select this compositor explicitly, regardless of the caller's session.
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    env.pop("WINIT_UNIX_BACKEND", None)
    config = output / "weston.ini"
    config_text = "[shell]\nlocking=false\n"
    if args.weston_root:
        root = args.weston_root.resolve(strict=True) / "usr"
        lib = root / "lib/x86_64-linux-gnu"
        modules = list(lib.glob("libweston-*/x11-backend.so"))
        if len(modules) != 1:
            parser.error("extracted root must contain one Debian amd64 libweston version")
        backend_dir = modules[0].parent
        env["LD_LIBRARY_PATH"] = f"{lib}:{lib}/weston:" + env.get("LD_LIBRARY_PATH", "")
        env["WESTON_DATA_DIR"] = str(root / "share/weston")
        env["WESTON_MODULE_MAP"] = (
            f"{args.backend}-backend.so={backend_dir}/{args.backend}-backend.so;"
            f"desktop-shell.so={lib}/weston/desktop-shell.so")
        config_text += (f"client={root}/libexec/weston-desktop-shell\n"
                        f"[input-method]\npath={root}/libexec/weston-keyboard\n")
        weston = str(root / "bin/weston")
    else:
        weston = shutil.which("weston")
        if not weston:
            parser.error("install Weston or provide --weston-root")
    config.write_text(config_text, encoding="utf-8")
    print(f"Wayland Vulkan evidence: {output}", flush=True)
    with tempfile.TemporaryDirectory(prefix="fragile-wayland-") as temporary:
        runtime = Path(temporary)
        env["XDG_RUNTIME_DIR"] = str(runtime)
        command = [weston, f"--backend={args.backend}", "--renderer=pixman",
                   "--shell=desktop-shell.so", "--socket=wayland-fragile",
                   "--idle-time=0", "--width=1280", "--height=900",
                   f"--config={config}", f"--log={output / 'weston.log'}"]
        failed = []
        with (output / "server-output.log").open("w", encoding="utf-8") as server_log:
            server = subprocess.Popen(command, env=env, stdout=server_log,
                                      stderr=subprocess.STDOUT, start_new_session=True)
            try:
                deadline = time.monotonic() + 15
                while not (runtime / "wayland-fragile").exists():
                    if server.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f"Weston did not create its socket; see {output}")
                    time.sleep(.05)
                client_env = dict(env, WAYLAND_DISPLAY="wayland-fragile")
                cases = [
                    ("handoff", [str(scripts / "check-vulkan-handoff.py"), "--binary", str(binary),
                                 "--output", str(output / "handoff")]),
                    *[(workload, [str(scripts / "profile-vulkan-live.py"), "--binary", str(binary),
                                  "--workload", workload, "--output", str(output / "live")])
                      for workload in ("about", "editor")],
                ]
                for name, arguments in cases:
                    with (output / f"{name}.log").open("w", encoding="utf-8") as log:
                        result = subprocess.run([sys.executable, *arguments], env=client_env,
                                                stdout=log, stderr=subprocess.STDOUT, timeout=600)
                    passed = result.returncode == 0
                    print(f"{'PASS' if passed else 'FAIL'} Wayland {name}", flush=True)
                    if not passed:
                        failed.append(name)
                if server.poll() is not None:
                    failed.append("compositor exited")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
        if failed:
            raise SystemExit(f"Wayland validation failed: {', '.join(failed)}; see {output}")


if __name__ == "__main__":
    main()
