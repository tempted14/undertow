"""Compare two Windows builds with isolated, muted native playback."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile


def measure(executable, fixture, output, name, backend, large_library):
    data = output / f"{name}-{backend}-data"
    data.mkdir()
    settings = {
        "discord": False,  # Never publish presence during an isolated run.
        "floating_lyrics": True,
        "everywhere": True,
        "lyric_glow": True,
        "lyric_cover_color": True,
        "lyric_motion_blur": True,
        "animated_lyrics": True,
        "cover_theme": True,
        "frosted_glass": True,
        "playlist_background": True,
        "show_sidebar": True,
        "show_covers": True,
        "fps": 120,
    }
    (data / "settings.json").write_text(json.dumps(settings), encoding="utf-8")
    if large_library:
        playlists = [
            {
                "id": f"fixture-{index}", "title": f"Evening collection {index + 1}",
                "description": "Generated local benchmark playlist", "count": 48,
                "duration": 8500, "image": f"local:{fixture.with_suffix('.png')}",
                "owned": True,
            }
            for index in range(800)
        ]
        (data / "playlists.json").write_text(json.dumps(playlists), encoding="utf-8")
    report = output / f"{name}-{backend}.json"
    args = [str(executable), "--background-benchmark", "--benchmark", "18", "--mute",
            "--open", str(fixture), "--window-size", "1600x1000", "--report", str(report)]
    if not large_library:
        args.append("--lyrics")
    if backend == "opengl":
        args.append("--gpu-benchmark")
    env = dict(os.environ, UNDERTOW_DATA_DIR=str(data))
    flags = subprocess.CREATE_NO_WINDOW | subprocess.BELOW_NORMAL_PRIORITY_CLASS
    with (output / f"{name}-{backend}.log").open("wb") as log:
        app = subprocess.Popen(args, env=env, stdout=log, stderr=subprocess.STDOUT, creationflags=flags)
        gpu_path = output / f"{name}-{backend}-gpu.json"
        gpu = None
        if backend == "opengl":
            sampler = Path(__file__).with_name("gpu-sample.ps1")
            gpu = subprocess.Popen(
                ["pwsh", "-NoProfile", "-File", str(sampler), "-TaskProcessId", str(app.pid),
                 "-OutputPath", str(gpu_path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                creationflags=flags,
            )
        if app.wait(timeout=55):
            raise RuntimeError(f"{name}/{backend} failed. See the case log.")
        if gpu:
            gpu.wait(timeout=30)
    payload = json.loads(report.read_text(encoding="utf-8"))
    details = payload["details"]
    if not details["playback_active"] or not details["floating_lyrics_in_process"]:
        raise RuntimeError(f"{name}/{backend} did not activate both playback and floating lyrics")
    result = {key: payload[key] for key in ("cpu_percent_one_core", "peak_working_set_bytes", "peak_private_bytes")}
    result.update(case=name, backend=backend, frames=details["ui_frames"], underruns=details["underrun_samples"])
    if gpu_path.is_file():
        samples = json.loads(gpu_path.read_text(encoding="utf-8-sig"))
        active = [row for row in samples if row["matched_instances"] and row["engine_percent_sum"] is not None]
        if active:
            result["gpu_engine_mean_pct"] = sum(row["engine_percent_sum"] for row in active) / len(active)
    print(json.dumps(result), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Requires the native Windows executable")
    args.output.mkdir(parents=True)
    output = args.output.resolve()
    series = {}
    fixtures = runpy.run_path(str(Path(__file__).with_name("ui-smoke.py")))["fixtures"]
    with tempfile.TemporaryDirectory(prefix="undertow-desktop-check-") as folder:
        fixture = fixtures(Path(folder))
        for name, executable in [("before", args.baseline), ("after", args.candidate)]:
            executable = executable.resolve(strict=True)
            results = []
            for case, backend, large in [("library", "software", True), ("lyrics", "software", False), ("lyrics", "opengl", False)]:
                results.append(measure(executable, fixture, output, f"{name}-{case}", backend, large))
            series[name] = {"sha256": hashlib.sha256(executable.read_bytes()).hexdigest(), "results": results}
    (output / "summary.json").write_text(json.dumps(series, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
